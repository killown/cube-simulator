# WGPU Cube Simulator

This is a raymarching playground built with Rust and WGPU. Instead of the usual triangle-based rasterization, this project uses fragment shaders to calculate signed distance fields (SDFs) in real-time. It's built for performance testing and seeing how hard we can push a GPU with purely mathematical geometry.

---



https://github.com/user-attachments/assets/f839f53b-9e11-4ce6-a3b9-cc3d5adc6076



### Installation and Usage

Make sure your Rust toolchain is current. Build and run with the release profile for actual performance:

    cargo build --release
    target/release/frame-test -- [ARGS]

### CLI Parameters

| Argument                 | Description                              | Default       |
| :----------------------- | :--------------------------------------- | :------------ |
| `-c, --cubes`            | Number of hollow cubes to march.         | 6             |
| `-s, --size`             | Radius/Scale of the objects.             | 0.5           |
| `--speed`                | Multiplier for rotation and oscillation. | 1.0           |
| `--red, --green, --blue` | RGB float components (0.0 to 1.0).       | 0.5, 0.8, 0.2 |

---

### Performance Note

    Top: Current FPS
    Middle: Max FPS
    Bottom: Min FPS

Raymarching is exponentially expensive based on the complexity of the `map()` function. Cranking the cube count will eventually tank your framerate. If the green counter in the top-left drops, you're hitting the limit of your hardware's throughput.
