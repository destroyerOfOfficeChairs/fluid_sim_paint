# Fluid Sim Paint

A GPU-accelerated fluid simulation painting application written in Rust.

This project implements a stable **Eulerian Fluid Simulation** on the GPU using `wgpu`. Unlike standard painting programs where pixels are static, the "paint" in this application is alive—it flows, swirls, and mixes based on real-time fluid dynamics (Navier-Stokes equations).

## Features

* **Real-Time Fluid Physics:** Solves Advection, Diffusion (Viscosity), and Pressure Projection (Incompressibility) at 60+ FPS.
* **GPU Compute Shaders:** All physics calculations happen on the GPU via WGSL compute shaders.
* **Interactive Tools:**
    * **Paint Brush:** Inject velocity and colored ink into the simulation.
    * **Smudge / Blender:** A specialized tool that mechanically mixes fluid colors, overcoming the natural "marbling" of laminar flow.
* **Dynamic Physics Controls:** Tweak viscosity, friction (velocity decay), and evaporation (ink decay) in real-time.
* **Infinite Canvas:** (Technically finite texture, but handles boundary conditions to prevent crashing).

## Controls

| Input | Action |
| :--- | :--- |
| **Left Mouse** | Apply Brush (Paint or Smudge) |
| **Delete / Backspace** | Clear Canvas |
| **GUI Panel** | Adjust Physics & Brush Settings |

## Installation & Build

Ensure you have [Rust and Cargo](https://rustup.rs/) installed.

```bash
# Clone the repository
git clone [https://github.com/yourusername/fluid_sim_paint.git](https://github.com/yourusername/fluid_sim_paint.git)
cd fluid_sim_paint

# Run in release mode (Recommended for smooth 60FPS)
cargo run --release
```

*Note:* Debug builds may be choppy due to the heavy computational load of the fluid solver.

## Architecture

The project is structured into three main modules:

`canvas_mod` (The Engine):
- Manages the wgpu Compute Pipelines (advect, diffuse, pressure, brush).
- Handles the "Ping-Pong" texture logic required for stable fluid simulation.
- Solves the Poisson equation for pressure using Jacobi Iteration.

`gui_mod` (The Interface):
- Built with egui and egui-wgpu.
- Controls simulation uniforms (Delta Time, Viscosity, Brush Radius) dynamically.

`state.rs` (The Window):
- Manages the winit event loop and wgpu surface configuration.

## Tech Stack

- **Language:** Rust
- **Graphics:** wgpu (WebGPU implementation)
- **GUI:** egui
- **Windowing:** winit

## Known Issues & Roadmap

This project is currently in the "Prototype -> Production" transition phase.

- **Marbling:** Colors tend to swirl rather than mix. The "Smudge" tool is a temporary workaround. True subtractive (CMY) color mixing is planned.
- **Wavy Lines:** Fast strokes can trigger Kelvin-Helmholtz instability. A "Lazy Mouse" smoothing algorithm is on the roadmap.
- **Refactoring:** The FluidSim struct is currently monolithic. Planned refactors include moving pipeline logic into dedicated structs and implementing a DoubleBuffer pattern for texture management.

License
MIT
