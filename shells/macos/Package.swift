// swift-tools-version: 6.2
import PackageDescription

// The Rust staticlib references Core Audio. A static archive does not carry those
// linker flags, so every target that links it has to.
let audioFrameworks: [LinkerSetting] = [
    .linkedFramework("AudioToolbox"),
    .linkedFramework("CoreAudio"),
    .linkedFramework("CoreFoundation"),
]

let package = Package(
    name: "LLaMP",
    platforms: [
        .macOS("26.0"),
    ],
    products: [
        .library(name: "LLaMPFFI", targets: ["LLaMPFFI"]),
        .executable(name: "LLaMP", targets: ["LLaMP"]),
    ],
    targets: [
        .binaryTarget(
            name: "llamp_ffi",
            path: "LlampFFI.xcframework"
        ),
        .target(
            name: "LLaMPFFI",
            dependencies: ["llamp_ffi"],
            linkerSettings: audioFrameworks
        ),
        .testTarget(
            name: "LLaMPFFITests",
            dependencies: ["LLaMPFFI", "llamp_ffi"],
            linkerSettings: audioFrameworks
        ),
        .target(
            name: "LLaMPApp",
            dependencies: ["LLaMPFFI", "llamp_ffi"],
            linkerSettings: audioFrameworks
        ),
        .executableTarget(
            name: "LLaMP",
            dependencies: ["LLaMPApp", "LLaMPFFI", "llamp_ffi"],
            path: "Sources/LLaMP",
            linkerSettings: audioFrameworks
        ),
        .target(
            name: "GoldenInflate",
            path: "Tests/GoldenInflate",
            publicHeadersPath: "include",
            linkerSettings: [.linkedLibrary("z")]
        ),
        .testTarget(
            name: "LLaMPAppTests",
            dependencies: ["LLaMPApp", "GoldenInflate"],
            linkerSettings: audioFrameworks
        ),
    ]
)
