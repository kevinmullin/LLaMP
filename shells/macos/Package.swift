// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "LLaMP",
    platforms: [
        .macOS("26.0"),
    ],
    products: [
        .library(name: "LLaMPFFI", targets: ["LLaMPFFI"]),
    ],
    targets: [
        .binaryTarget(
            name: "llamp_ffi",
            path: "LlampFFI.xcframework"
        ),
        .target(
            name: "LLaMPFFI",
            dependencies: ["llamp_ffi"]
        ),
        .testTarget(
            name: "LLaMPFFITests",
            dependencies: ["LLaMPFFI", "llamp_ffi"]
        ),
    ]
)
