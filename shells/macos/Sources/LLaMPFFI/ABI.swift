import llamp_ffi

/// Swift-shaped poll of the C frame counter. This package is not the ABI.
public enum FrameCounter {
    public static func poll() -> UInt64 {
        llamp_counter_poll()
    }
}
