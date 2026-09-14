import Synchronization
import XCTest
import llamp_ffi
@testable import LLaMPFFI

final class CounterPollTests: XCTestCase {
    func testPollsAt60HzForOneSecondAndSeesMonotonicValue() {
        let stop = Atomic(false)
        let publisher = Thread {
            while !stop.load(ordering: .relaxed) {
                llamp_counter_publish()
            }
        }
        publisher.start()
        defer {
            stop.store(true, ordering: .relaxed)
            while !publisher.isFinished {
                Thread.sleep(forTimeInterval: 0.001)
            }
        }

        var previous = FrameCounter.poll()
        var samples = [previous]
        for _ in 0..<60 {
            Thread.sleep(forTimeInterval: 1.0 / 60.0)
            let value = FrameCounter.poll()
            XCTAssertGreaterThanOrEqual(value, previous)
            previous = value
            samples.append(value)
        }
        XCTAssertEqual(samples.count, 61)
        XCTAssertGreaterThan(samples.last!, samples.first!)
    }
}
