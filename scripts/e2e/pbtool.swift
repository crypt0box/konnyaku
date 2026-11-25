import AppKit
import CryptoKit

let pb = NSPasteboard.general
let args = CommandLine.arguments

func save(_ path: String) {
  let items: [[String: Data]] = (pb.pasteboardItems ?? []).map { item in
    var entry: [String: Data] = [:]
    for t in item.types { if let d = item.data(forType: t) { entry[t.rawValue] = d } }
    return entry
  }
  let data = try! PropertyListSerialization.data(fromPropertyList: items, format: .binary, options: 0)
  try! data.write(to: URL(fileURLWithPath: path))
  print("saved \(items.count) items")
}

func restore(_ path: String) {
  let data = try! Data(contentsOf: URL(fileURLWithPath: path))
  let items = try! PropertyListSerialization.propertyList(from: data, format: nil) as! [[String: Data]]
  pb.clearContents()
  pb.writeObjects(items.map { entry in
    let item = NSPasteboardItem()
    for (t, d) in entry { item.setData(d, forType: NSPasteboard.PasteboardType(t)) }
    return item
  })
  print("restored \(items.count) items")
}

func setHTML(_ path: String, plain: String) {
  let html = try! String(contentsOfFile: path, encoding: .utf8)
  pb.clearContents()
  let item = NSPasteboardItem()
  item.setString(html, forType: .html)
  item.setString(plain, forType: .string)
  pb.writeObjects([item])
  print("set html (\(html.count) chars), changeCount \(pb.changeCount)")
}

/// One line per item and type with the data's SHA-256, so two clipboards compare byte for byte.
func digest() {
  for (i, item) in (pb.pasteboardItems ?? []).enumerated() {
    for t in item.types.map(\.rawValue).sorted() {
      let d = item.data(forType: NSPasteboard.PasteboardType(t)) ?? Data()
      let hash = SHA256.hash(data: d).map { String(format: "%02x", $0) }.joined()
      print(i, t, d.count, hash)
    }
  }
}

/// Two items with several types, including a private one, standing in for whatever the user copied.
func sentinel() {
  let png = Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==")!
  let first = NSPasteboardItem()
  first.setString("konnyaku clipboard sentinel", forType: .string)
  first.setString("<b>konnyaku clipboard sentinel</b>", forType: .html)
  first.setData(Data([0, 1, 2, 254, 255]), forType: NSPasteboard.PasteboardType("dev.konnyaku.sentinel"))
  let second = NSPasteboardItem()
  second.setData(png, forType: .png)
  pb.clearContents()
  pb.writeObjects([first, second])
  print("set sentinel, changeCount \(pb.changeCount)")
}

switch args[1] {
case "save": save(args[2])
case "restore": restore(args[2])
case "html": setHTML(args[2], plain: "plain fallback")
case "digest": digest()
case "sentinel": sentinel()
default: print("usage: pbtool save|restore <plist> | html <file> | digest | sentinel")
}
