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
    private var equalizer: EqWindow?
    private var playlist: PlaylistWindow?
    private var browser: BrowserWindow?
    private var visualizer: VisWindow?
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
        window.onOpenEqualizer = { [weak self] in self?.openEqualizer() }
        window.onOpenPlaylist = { [weak self] in self?.openPlaylist() }
        window.onOpenBrowser = { [weak self] in self?.openBrowser() }
        window.onOpenVisualizer = { [weak self] in self?.openVisualizer() }
        if let skin = argumentValue("--skin"), let data = try? Data(contentsOf: URL(fileURLWithPath: skin)), !data.isEmpty {
            window.loadSkin(data)
        }
        if let screen = NSScreen.main {
            var frame = window.frame
            frame.origin.x = screen.visibleFrame.midX - frame.width / 2
            frame.origin.y = screen.visibleFrame.midY
            window.setFrameOrigin(frame.origin)
        }
        if CommandLine.arguments.contains("--show-eq") {
            openEqualizer()
        }
        if CommandLine.arguments.contains("--show-playlist") {
            openPlaylist()
        }
        if CommandLine.arguments.contains("--show-browser") {
            openBrowser()
        }
        if CommandLine.arguments.contains("--show-vis") {
            openVisualizer()
        }
        if let track = argumentValue("--play") {
            let refs = argumentValue("--refs") ?? FileManager.default.temporaryDirectory.path
            _ = track.withCString { trackPtr in
                refs.withCString { refsPtr in
                    llamp_budget_prepare(trackPtr, refsPtr)
                }
            }
        }
        NSApp.activate(ignoringOtherApps: true)
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

    private func openEqualizer() {
        EqStore.install()
        if equalizer == nil {
            let eq = EqWindow()
            equalizer = eq
            WindowDock.shared.attach(main: window, equalizer: eq)
        }
        WindowDock.shared.showEqualizer()
    }

    private func openPlaylist() {
        if playlist == nil {
            let window = PlaylistWindow()
            playlist = window
            WindowDock.shared.attach(playlist: window)
        }
        playlist?.orderFront(nil)
    }

    private func openBrowser() {
        if browser == nil {
            let window = BrowserWindow()
            browser = window
            WindowDock.shared.attach(browser: window)
        }
        browser?.reloadGranted()
        browser?.orderFront(nil)
    }

    private func openVisualizer() {
        if visualizer == nil {
            let window = VisWindow()
            visualizer = window
            WindowDock.shared.attach(visualizer: window)
        }
        visualizer?.orderFront(nil)
    }
}

private struct BudgetScene {
    let wsz: Data
    let track: String
    let refs: String
}

private func argumentValue(_ flag: String) -> String? {
    let args = CommandLine.arguments
    guard let index = args.firstIndex(of: flag), args.count > index + 1 else {
        return nil
    }
    return args[index + 1]
}

private func budgetArguments() -> BudgetScene? {
    let args = CommandLine.arguments
    guard let index = args.firstIndex(of: "--budget-scene"), args.count > index + 3 else {
        return nil
    }
    let wsz = (try? Data(contentsOf: URL(fileURLWithPath: args[index + 1]))) ?? Data()
    return BudgetScene(wsz: wsz, track: args[index + 2], refs: args[index + 3])
}
