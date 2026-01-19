use wgpu::{BindGroup, CommandEncoder, ComputePipeline};

pub fn record_compute_pass(
    encoder: &mut CommandEncoder,
    pipeline: &ComputePipeline,
    bind_group_a: &BindGroup,
    bind_group_b: &BindGroup,
    frame_num: usize,
    sim_width: u32,
    sim_height: u32,
) {
    let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("Compute Pass"),
        timestamp_writes: None,
    });
    compute_pass.set_pipeline(pipeline);

    // Ping-Pong Logic
    if frame_num % 2 == 0 {
        // Even Frame: Read A -> Write B
        compute_pass.set_bind_group(0, bind_group_a, &[]);
    } else {
        // Odd Frame: Read B -> Write A
        compute_pass.set_bind_group(0, bind_group_b, &[]);
    }

    // Dynamic Dispatch Calculation
    let workgroup_size = 16;
    let x_groups = (sim_width + workgroup_size - 1) / workgroup_size;
    let y_groups = (sim_height + workgroup_size - 1) / workgroup_size;

    compute_pass.dispatch_workgroups(x_groups, y_groups, 1);
}
