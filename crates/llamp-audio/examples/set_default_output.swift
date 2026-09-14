import CoreAudio
import Foundation

// Sets or prints the system default output device by its CoreAudio name.
// Usage: set_default_output [--get] [name]

func fail(_ message: String) -> Never {
    fputs("\(message)\n", stderr)
    exit(1)
}

func address(_ selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress(
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
}

func deviceName(_ id: AudioDeviceID) -> String? {
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyDeviceNameCFString,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    var name: CFString?
    var size = UInt32(MemoryLayout<CFString?>.size)
    let status = withUnsafeMutablePointer(to: &name) { ptr in
        AudioObjectGetPropertyData(id, &addr, 0, nil, &size, ptr)
    }
    guard status == noErr, let name else { return nil }
    return name as String
}

func hasOutput(_ id: AudioDeviceID) -> Bool {
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyStreams,
        mScope: kAudioDevicePropertyScopeOutput,
        mElement: kAudioObjectPropertyElementMain
    )
    var size: UInt32 = 0
    let status = AudioObjectGetPropertyDataSize(id, &addr, 0, nil, &size)
    return status == noErr && size >= UInt32(MemoryLayout<AudioStreamID>.size)
}

func outputDevices() -> [(AudioDeviceID, String)] {
    var addr = address(kAudioHardwarePropertyDevices)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size) == noErr else {
        return []
    }
    let count = Int(size) / MemoryLayout<AudioDeviceID>.size
    var ids = [AudioDeviceID](repeating: 0, count: count)
    guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size, &ids) == noErr else {
        return []
    }
    return ids.compactMap { id in
        guard hasOutput(id), let name = deviceName(id) else { return nil }
        return (id, name)
    }
}

func defaultOutputID() -> AudioDeviceID? {
    var addr = address(kAudioHardwarePropertyDefaultOutputDevice)
    var id = AudioDeviceID(0)
    var size = UInt32(MemoryLayout<AudioDeviceID>.size)
    let status = AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size, &id)
    guard status == noErr, id != 0 else { return nil }
    return id
}

let args = Array(CommandLine.arguments.dropFirst())
if args.first == "--get" {
    guard let id = defaultOutputID(), let name = deviceName(id) else {
        fail("no default output")
    }
    print(name)
    exit(0)
}

guard let wanted = args.first, !wanted.isEmpty else {
    fail("usage: set_default_output [--get] <name>")
}
guard let match = outputDevices().first(where: { $0.1 == wanted }) else {
    fail("no output named \(wanted)")
}

var id = match.0
var addr = address(kAudioHardwarePropertyDefaultOutputDevice)
let size = UInt32(MemoryLayout<AudioDeviceID>.size)
let status = AudioObjectSetPropertyData(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, size, &id)
guard status == noErr else {
    fail("set default output failed: \(status)")
}
print(wanted)
