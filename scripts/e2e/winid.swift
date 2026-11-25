import CoreGraphics

// Windows of the owner app (default konnyaku), front to back: id, title (spaces become _), then the bounds in global points (top-left origin, the space CGEvent positions use).
let owner = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "konnyaku"
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
for w in windows where (w[kCGWindowOwnerName as String] as? String) == owner && (w[kCGWindowLayer as String] as? Int) == 0 {
  let b = w[kCGWindowBounds as String] as! [String: Double]
  let title = (w[kCGWindowName as String] as? String).map { $0.isEmpty ? "-" : $0.replacingOccurrences(of: " ", with: "_") } ?? "-"
  print(w[kCGWindowNumber as String]!, title,
        Int(b["X"]!), Int(b["Y"]!), Int(b["Width"]!), Int(b["Height"]!))
}
