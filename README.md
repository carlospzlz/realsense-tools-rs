# RealSense Tools in Rust

This project contains a suite of tools to work with Intel RealSense cameras
using the `realsense-rust` crate. The tools provide visualization and
interaction with camera streams and sensors.

## Tools

### `realsense-viewer`

A tool to view streams, images, and sensor data from a RealSense camera. It
offers a graphical interface to control and visualize color, depth, infrared
streams, and data from the motion module.

### `realsense-3d-viewer`

A simple 3D visualization tool that reconstructs a 3D cubes-based mesh using
the depth and an infrared stream. This tool allows basic interaction such as
rotating, zooming, and panning.

## Build

```sh
cargo build --release
```

## Usage

```sh
cargo run --bin realsense-viewer
cargo run --bin realsense-3d-viewer
```

## Dependencies

- `realsense-rust`: Interface with RealSense devices.
- `egui`: GUI framework for Rust.
- `glow`: OpenGL renderer for 3D visualization.
