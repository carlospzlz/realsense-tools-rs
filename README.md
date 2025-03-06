# RealSense Tools in Rust

This project contains a suite of tools to work with Intel RealSense cameras
using the [realsense_rust](https://docs.rs/realsense-rust) crate. The tools
provide visualization and interaction with camera streams and sensors.

## Tools

### `realsense-viewer`

A tool to view streams, images, and sensor data from a RealSense camera. It
offers a graphical interface to control and visualize color, depth, infrared
streams, and data from the motion module.

https://github.com/user-attachments/assets/93a87348-419a-4522-9850-2d0f98e299c0

### `realsense-3d-viewer`

A simple 3D visualization tool that reconstructs a 3D cubes-based mesh using
the depth and an infrared stream. This tool allows basic interaction such as
rotating, zooming, and panning.

https://github.com/user-attachments/assets/96a94e77-5ab2-4ea1-9123-8e40bddb2c52

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

- [realsense_rust](https://docs.rs/realsense-rust): Interface with RealSense devices.
- [egui](https://docs.rs/egui): GUI framework for Rust.
- [glow](https://docs.rs/glow): OpenGL renderer for 3D visualization.
