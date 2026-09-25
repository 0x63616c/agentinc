// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "AgentIncGhosttyBridge",
    platforms: [.macOS(.v15)],
    products: [
        .library(name: "AgentIncGhosttyBridge", type: .dynamic, targets: ["AgentIncGhosttyBridge"]),
    ],
    dependencies: [
        .package(url: "https://github.com/Lakr233/libghostty-spm.git", exact: "1.6.20260922"),
    ],
    targets: [
        .target(
            name: "AgentIncGhosttyBridge",
            dependencies: [.product(name: "GhosttyTerminal", package: "libghostty-spm")]
        ),
        .testTarget(name: "AgentIncGhosttyBridgeTests", dependencies: ["AgentIncGhosttyBridge"]),
    ]
)
