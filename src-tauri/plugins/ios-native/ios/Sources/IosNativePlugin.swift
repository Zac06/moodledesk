import QuickLook
import Tauri
import UIKit

struct OpenArgs: Decodable {
  let path: String
}

class IosNativePlugin: Plugin, QLPreviewControllerDataSource {
  private var file: NSURL?

  /// Shows a downloaded file in Quick Look, the system viewer: PDFs, images, Office files, text and more.
  /// Its Share button gives "Open in…" and "Save to Files".
  @objc func openFile(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(OpenArgs.self)
    let url = URL(fileURLWithPath: args.path)
    guard FileManager.default.fileExists(atPath: url.path) else {
      invoke.reject("The file is not there anymore")
      return
    }

    DispatchQueue.main.async {
      guard let presenter = self.manager.viewController else {
        invoke.reject("No window to show the file in")
        return
      }
      self.file = url as NSURL
      let preview = QLPreviewController()
      preview.dataSource = self
      presenter.present(preview, animated: true) { invoke.resolve() }
    }
  }

  func numberOfPreviewItems(in controller: QLPreviewController) -> Int {
    return file == nil ? 0 : 1
  }

  func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem {
    return file!
  }
}

@_cdecl("init_plugin_ios_native")
func initPlugin() -> Plugin {
  return IosNativePlugin()
}
