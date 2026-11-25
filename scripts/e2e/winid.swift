import CoreGraphics

let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
for w in windows where (w[kCGWindowOwnerName as String] as? String) == "konnyaku" {
  print(w[kCGWindowNumber as String]!, w[kCGWindowName as String] ?? "-")
}
