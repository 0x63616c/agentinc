import AppKit
import Vision

let path = CommandLine.arguments[1]
let image = NSImage(contentsOfFile: path)!
var rect = CGRect(origin: .zero, size: image.size)
let cgImage = image.cgImage(forProposedRect: &rect, context: nil, hints: nil)!
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
try VNImageRequestHandler(cgImage: cgImage).perform([request])
for observation in request.results ?? [] {
    if let text = observation.topCandidates(1).first?.string {
        print(text)
    }
}
