import AppKit

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

switch args[1] {
case "save": save(args[2])
case "restore": restore(args[2])
case "html": setHTML(args[2], plain: "plain fallback")
default: print("usage: pbtool save|restore <plist> | html <file>")
}
