#import <Foundation/Foundation.h>

#include <stdatomic.h>
#include <stdint.h>

#if __MAC_OS_X_VERSION_MAX_ALLOWED >= 140200
#import <CoreAudio/AudioHardware.h>
#import <CoreAudio/AudioHardwareTapping.h>
#import <CoreAudio/CATapDescription.h>

#include <math.h>
#include <string.h>

#define DS_AUDIO_CAPTURE_HAS_TAPS 1
#else
#define DS_AUDIO_CAPTURE_HAS_TAPS 0
#endif

enum {
    DS_AUDIO_CAPTURE_IDLE = 0,
    DS_AUDIO_CAPTURE_RUNNING = 1,
    DS_AUDIO_CAPTURE_UNSUPPORTED = 2,
    DS_AUDIO_CAPTURE_FAILED = 3,
};

#if DS_AUDIO_CAPTURE_HAS_TAPS

typedef struct {
    AudioObjectID tap_id;
    AudioObjectID aggregate_id;
    AudioDeviceIOProcID io_proc;
    AudioStreamBasicDescription format;
    float low_pass_state;
} DSAudioCapture;

static DSAudioCapture g_capture = {0};
static CATapDescription *g_tap_description;
static _Atomic uint64_t g_levels = 0;
static _Atomic uint64_t g_sequence = 0;
static _Atomic int g_status = DS_AUDIO_CAPTURE_IDLE;
static _Atomic int32_t g_last_error = 0;
static _Atomic bool g_accepting_audio = false;

static void ds_reset_levels(void) {
    atomic_store_explicit(&g_levels, 0, memory_order_release);
    atomic_fetch_add_explicit(&g_sequence, 1, memory_order_release);
}

static uint16_t ds_encode_level(float value) {
    if (!isfinite(value) || value <= 0.0f) {
        return 0;
    }
    if (value >= 1.0f) {
        return UINT16_MAX;
    }
    return (uint16_t)lrintf(value * (float)UINT16_MAX);
}

static float ds_read_sample(const uint8_t *bytes, UInt32 bits_per_channel, bool is_float) {
    if (is_float && bits_per_channel == 32) {
        float sample;
        memcpy(&sample, bytes, sizeof(sample));
        return sample;
    }
    if (is_float && bits_per_channel == 64) {
        double sample;
        memcpy(&sample, bytes, sizeof(sample));
        return (float)sample;
    }
    if (!is_float && bits_per_channel == 16) {
        int16_t sample;
        memcpy(&sample, bytes, sizeof(sample));
        return (float)sample / 32768.0f;
    }
    if (!is_float && bits_per_channel == 32) {
        int32_t sample;
        memcpy(&sample, bytes, sizeof(sample));
        return (float)((double)sample / 2147483648.0);
    }
    return 0.0f;
}

static OSStatus ds_audio_io_proc(
    AudioObjectID in_device,
    const AudioTimeStamp *in_now,
    const AudioBufferList *in_input_data,
    const AudioTimeStamp *in_input_time,
    AudioBufferList *out_output_data,
    const AudioTimeStamp *in_output_time,
    void *in_client_data
) {
    (void)in_device;
    (void)in_now;
    (void)in_input_time;
    (void)out_output_data;
    (void)in_output_time;

    DSAudioCapture *capture = in_client_data;
    if (!atomic_load_explicit(&g_accepting_audio, memory_order_acquire) ||
        capture == NULL || in_input_data == NULL ||
        capture->format.mFormatID != kAudioFormatLinearPCM) {
        return noErr;
    }

    const UInt32 bits_per_channel = capture->format.mBitsPerChannel;
    const bool is_float = (capture->format.mFormatFlags & kAudioFormatFlagIsFloat) != 0;
    const UInt32 sample_bytes = bits_per_channel / 8;
    if (sample_bytes == 0 ||
        !((is_float && (bits_per_channel == 32 || bits_per_channel == 64)) ||
          (!is_float && (bits_per_channel == 16 || bits_per_channel == 32)))) {
        return noErr;
    }

    double low_energy = 0.0;
    double high_energy = 0.0;
    uint64_t sample_count = 0;
    const float low_pass_alpha = 0.03f;

    for (UInt32 buffer_index = 0; buffer_index < in_input_data->mNumberBuffers; ++buffer_index) {
        const AudioBuffer buffer = in_input_data->mBuffers[buffer_index];
        if (buffer.mData == NULL || buffer.mDataByteSize < sample_bytes) {
            continue;
        }

        const uint8_t *bytes = buffer.mData;
        const UInt32 count = buffer.mDataByteSize / sample_bytes;
        for (UInt32 sample_index = 0; sample_index < count; ++sample_index) {
            const float sample = ds_read_sample(
                bytes + ((size_t)sample_index * sample_bytes),
                bits_per_channel,
                is_float
            );
            const float low = capture->low_pass_state +
                low_pass_alpha * (sample - capture->low_pass_state);
            capture->low_pass_state = low;
            const float high = sample - low;
            low_energy += (double)low * (double)low;
            high_energy += (double)high * (double)high;
            ++sample_count;
        }
    }

    if (sample_count > 0) {
        const uint16_t low = ds_encode_level((float)sqrt(low_energy / (double)sample_count));
        const uint16_t high = ds_encode_level((float)sqrt(high_energy / (double)sample_count));
        const uint64_t levels = ((uint64_t)low << 16) | (uint64_t)high;
        atomic_store_explicit(&g_levels, levels, memory_order_release);
        atomic_fetch_add_explicit(&g_sequence, 1, memory_order_release);
    }

    return noErr;
}

static OSStatus ds_read_tap_format(
    AudioObjectID tap_id,
    AudioStreamBasicDescription *out_format
) {
    AudioObjectPropertyAddress address = {
        kAudioTapPropertyFormat,
        kAudioObjectPropertyScopeGlobal,
        kAudioObjectPropertyElementMain,
    };
    UInt32 size = sizeof(*out_format);
    return AudioObjectGetPropertyData(tap_id, &address, 0, NULL, &size, out_format);
}

static void ds_teardown_capture(void) {
    atomic_store_explicit(&g_accepting_audio, false, memory_order_release);
    ds_reset_levels();

    if (g_capture.aggregate_id != kAudioObjectUnknown && g_capture.io_proc != NULL) {
        (void)AudioDeviceStop(g_capture.aggregate_id, g_capture.io_proc);
        (void)AudioDeviceDestroyIOProcID(g_capture.aggregate_id, g_capture.io_proc);
    }
    g_capture.io_proc = NULL;

    if (g_capture.aggregate_id != kAudioObjectUnknown) {
        (void)AudioHardwareDestroyAggregateDevice(g_capture.aggregate_id);
    }
    g_capture.aggregate_id = kAudioObjectUnknown;

    if (g_capture.tap_id != kAudioObjectUnknown) {
        (void)AudioHardwareDestroyProcessTap(g_capture.tap_id);
    }
    g_capture.tap_id = kAudioObjectUnknown;
    g_capture.format = (AudioStreamBasicDescription){0};
    g_capture.low_pass_state = 0.0f;
    g_tap_description = nil;
}

static int32_t ds_fail(OSStatus error) {
    ds_teardown_capture();
    atomic_store_explicit(&g_last_error, error, memory_order_release);
    atomic_store_explicit(&g_status, DS_AUDIO_CAPTURE_FAILED, memory_order_release);
    return error == noErr ? -1 : error;
}

int32_t ds_audio_capture_start(void) {
    if (@available(macOS 14.2, *)) {
        ds_teardown_capture();
        atomic_store_explicit(&g_last_error, 0, memory_order_release);

        @autoreleasepool {
            CATapDescription *description =
                [[CATapDescription alloc] initStereoGlobalTapButExcludeProcesses:@[]];
            description.name = @"DualSenseTUI System Audio";
            description.UUID = [NSUUID UUID];
            description.privateTap = YES;
            description.muteBehavior = CATapUnmuted;

            OSStatus status = AudioHardwareCreateProcessTap(description, &g_capture.tap_id);
            if (status != noErr || g_capture.tap_id == kAudioObjectUnknown) {
                return ds_fail(status);
            }
            g_tap_description = description;

            status = ds_read_tap_format(g_capture.tap_id, &g_capture.format);
            if (status != noErr) {
                return ds_fail(status);
            }

            NSDictionary *tap = @{
                @kAudioSubTapUIDKey: description.UUID.UUIDString,
                @kAudioSubTapDriftCompensationKey: @YES,
            };
            NSDictionary *aggregate = @{
                @kAudioAggregateDeviceNameKey: @"DualSenseTUI System Audio",
                @kAudioAggregateDeviceUIDKey: [NSUUID UUID].UUIDString,
                @kAudioAggregateDeviceTapListKey: @[tap],
                @kAudioAggregateDeviceTapAutoStartKey: @NO,
                @kAudioAggregateDeviceIsPrivateKey: @YES,
            };
            status = AudioHardwareCreateAggregateDevice(
                (__bridge CFDictionaryRef)aggregate,
                &g_capture.aggregate_id
            );
            if (status != noErr || g_capture.aggregate_id == kAudioObjectUnknown) {
                return ds_fail(status);
            }

            status = AudioDeviceCreateIOProcID(
                g_capture.aggregate_id,
                ds_audio_io_proc,
                &g_capture,
                &g_capture.io_proc
            );
            if (status != noErr || g_capture.io_proc == NULL) {
                return ds_fail(status);
            }

            atomic_store_explicit(&g_accepting_audio, true, memory_order_release);
            status = AudioDeviceStart(g_capture.aggregate_id, g_capture.io_proc);
            if (status != noErr) {
                return ds_fail(status);
            }
        }

        atomic_store_explicit(&g_status, DS_AUDIO_CAPTURE_RUNNING, memory_order_release);
        return 0;
    }

    atomic_store_explicit(&g_last_error, 0, memory_order_release);
    atomic_store_explicit(&g_status, DS_AUDIO_CAPTURE_UNSUPPORTED, memory_order_release);
    return -1;
}

void ds_audio_capture_stop(void) {
    ds_teardown_capture();
    atomic_store_explicit(&g_last_error, 0, memory_order_release);
    atomic_store_explicit(&g_status, DS_AUDIO_CAPTURE_IDLE, memory_order_release);
}

int32_t ds_audio_capture_state(void) {
    return atomic_load_explicit(&g_status, memory_order_acquire);
}

int32_t ds_audio_capture_last_error(void) {
    return atomic_load_explicit(&g_last_error, memory_order_acquire);
}

uint64_t ds_audio_capture_levels(void) {
    return atomic_load_explicit(&g_levels, memory_order_acquire);
}

uint64_t ds_audio_capture_sequence(void) {
    return atomic_load_explicit(&g_sequence, memory_order_acquire);
}

#else

int32_t ds_audio_capture_start(void) {
    return -1;
}

void ds_audio_capture_stop(void) {}

int32_t ds_audio_capture_state(void) {
    return DS_AUDIO_CAPTURE_UNSUPPORTED;
}

int32_t ds_audio_capture_last_error(void) {
    return 0;
}

uint64_t ds_audio_capture_levels(void) {
    return 0;
}

uint64_t ds_audio_capture_sequence(void) {
    return 0;
}

#endif
