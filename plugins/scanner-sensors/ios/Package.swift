// swift-tools-version:5.9

import PackageDescription

let package = Package(
  name: "tauri-plugin-scanner-sensors",
  platforms: [
    .iOS(.v15),
  ],
  products: [
    .library(
      name: "tauri-plugin-scanner-sensors",
      type: .static,
      targets: ["tauri-plugin-scanner-sensors"]
    )
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(
      name: "tauri-plugin-scanner-sensors",
      dependencies: [
        .byName(name: "Tauri")
      ],
      path: "Sources"
    )
  ]
)
