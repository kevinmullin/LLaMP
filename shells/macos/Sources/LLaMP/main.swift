import AppKit
import Darwin
import LLaMPApp
import llamp_ffi

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.regular)
app.run()

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let window = MainWindow()
    private let started = ContinuousClock.now

    func applicationDidFinishLaunching(_ notification: Notification) {
        window.makeKeyAndOrderFront(nil)
        window.contentView?.display()
        if CommandLine.arguments.contains("--paint-and-exit") {
            let elapsed = started.duration(to: .now)
            let ms = Double(elapsed.components.seconds) * 1000 + Double(elapsed.components.attoseconds) / 1_000_000_000_000_000
            print("painted \(Int(ms))")
            fflush(stdout)
            NSApp.terminate(nil)
            return
        }
        if let scene = budgetArguments() {
            window.loadSkin(scene.wsz)
            NSApp.activate(ignoringOtherApps: true)
            window.makeKeyAndOrderFront(nil)
            let startedPlay = scene.track.withCString { track in
                scene.refs.withCString { refs in
                    llamp_budget_prepare(track, refs)
                }
            }
            guard startedPlay == LLAMP_OK else {
                fputs("budget scene failed to start playback\n", stderr)
                exit(1)
            }
            for _ in 0..<50 {
                let snap = llamp_playback_poll()
                if snap.transport == 1 && snap.position_frames > 0 && llamp_reference_count() >= 1000 {
                    print("budget-ready refs=\(llamp_reference_count())")
                    fflush(stdout)
                    return
                }
                Thread.sleep(forTimeInterval: 0.1)
            }
            fputs("budget scene did not reach a playing track\n", stderr)
            exit(1)
        }
    }
}

private struct BudgetScene {
    let wsz: Data
    let track: String
    let refs: String
}

private func budgetArguments() -> BudgetScene? {
    let args = CommandLine.arguments
    guard let index = args.firstIndex(of: "--budget-scene"), args.count > index + 3 else {
        return nil
    }
    let wsz = (try? Data(contentsOf: URL(fileURLWithPath: args[index + 1]))) ?? Data()
    return BudgetScene(wsz: wsz, track: args[index + 2], refs: args[index + 3])
}
