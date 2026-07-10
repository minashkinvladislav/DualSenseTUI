import AppKit
import Foundation

let arguments = CommandLine.arguments
guard arguments.count == 3 else {
    fputs("Usage: generate-macos-icon.swift <source.png> <output.png>\n", stderr)
    exit(2)
}

let sourceURL = URL(fileURLWithPath: arguments[1])
let outputURL = URL(fileURLWithPath: arguments[2])
guard let sourceImage = NSImage(contentsOf: sourceURL) else {
    fputs("Could not load icon source image: \(sourceURL.path)\n", stderr)
    exit(1)
}

let canvasSize: CGFloat = 1024
let image = NSImage(size: NSSize(width: canvasSize, height: canvasSize))
image.lockFocus()
NSGraphicsContext.current?.imageInterpolation = .high

NSColor.white.setFill()
NSBezierPath(rect: NSRect(x: 0, y: 0, width: canvasSize, height: canvasSize)).fill()

let destinationRect = NSRect(x: 62, y: 62, width: 900, height: 900)
let sourceRect = NSRect(origin: .zero, size: sourceImage.size)
sourceImage.draw(in: destinationRect, from: sourceRect, operation: .copy, fraction: 1)
image.unlockFocus()

guard let tiff = image.tiffRepresentation,
      let bitmap = NSBitmapImageRep(data: tiff),
      let png = bitmap.representation(using: .png, properties: [:]) else {
    fputs("Could not encode macOS icon PNG.\n", stderr)
    exit(1)
}

try png.write(to: outputURL)
