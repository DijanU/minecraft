# Raytracer Minecraft

This project is a feature-rich ray tracer built with Rust, inspired by the aesthetics of Minecraft. It renders a 3D scene composed of various types of blocks, implementing advanced graphics techniques like reflection, refraction, textures, and dynamic lighting.

## Showcase

Here is a video demonstrating the ray tracer in action:

[Project Demo Video](https://uvggt-my.sharepoint.com/:v:/g/personal/pad23663_uvg_edu_gt/IQCmdRbmCCdiT6MlIWk8EYyqAXLmt96-K79c1gRe2cesPm4?nav=eyJyZWZlcnJhbEluZm8iOnsicmVmZXJyYWxBcHAiOiJPbmVEcml2ZUZvckJ1c2luZXNzIiwicmVmZXJyYWxBcHBQbGF0Zm9ybSI6IldlYiIsInJlZmVycmFsTW9kZSI6InZpZXciLCJyZWZlcnJhbFZpZXciOiJNeUZpbGVzTGlua0NvcHkifX0&e=KblSV8)

## Features

- **Minecraft-Inspired Scene**: The world is built from cubes with various materials like grass, stone, wood, and more.
- **Advanced Materials**:
    - **Reflection**: Objects like diamond and obsidian have reflective surfaces.
    - **Refraction**: Simulates light passing through transparent materials like glass and water.
    - **Emission**: Emissive blocks like magma and torches cast their own light.
- **Texturing**: Blocks are textured using image files from the `assets` directory.
- **Dynamic Day/Night Cycle**: A moving sun simulates the time of day, affecting the scene's lighting and shadows.
- **Skybox**: A skybox provides a realistic and immersive background.
- **Interactive Camera**:
    - **Orbit**: Rotate the camera around the scene using the arrow keys.
    - **Zoom**: Zoom in and out using the 'A' and 'D' keys.
    - **Pan**: Move the camera up and down with the 'W' and 'S' keys.
    - **Auto-Rotate**: Toggle a slow automatic rotation with the SPACE bar.
- **Performance Optimizations**:
    - **Parallelism**: Uses the `rayon` crate to cast rays in parallel, leveraging multiple CPU cores.
    - **BVH Acceleration**: Implements a Bounding Volume Hierarchy (BVH) to speed up ray-object intersection tests.
- **Performance Logging**: Frame rate and render times are logged to `performance_log.txt`.

## Setup and Running

### Prerequisites

- [Rust and Cargo](https://www.rust-lang.org/tools/install)

### Running the Project

To run the application in release mode for the best performance, execute the following command from the project root:

```bash
cargo run --release
```

## Dependencies

This project relies on the following main crates:

- `raylib`: For creating the window, handling user input, and displaying the rendered image.
- `rayon`: For parallel iteration to accelerate the rendering process.
- `bvh`: For the Bounding Volume Hierarchy implementation.
- `nalgebra`: For linear algebra operations (vectors and matrices).
