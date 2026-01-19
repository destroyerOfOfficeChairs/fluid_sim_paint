use wgpu::{BindGroup, Buffer, CommandEncoder, IndexFormat, RenderPipeline, TextureView};

pub fn record_render_pass(
    encoder: &mut CommandEncoder,
    view: &TextureView,
    pipeline: &RenderPipeline,
    bind_group_a: &BindGroup,
    bind_group_b: &BindGroup,
    vertex_buffer: &Buffer,
    index_buffer: &Buffer,
    num_indices: u32,
    frame_num: usize,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    });

    render_pass.set_pipeline(pipeline);

    // Determine which texture holds the "latest" result to draw
    // If frame 0 (Even), we read A -> wrote B. So B has the latest data.
    if frame_num % 2 == 0 {
        render_pass.set_bind_group(0, bind_group_b, &[]);
    } else {
        render_pass.set_bind_group(0, bind_group_a, &[]);
    }

    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);
    render_pass.draw_indexed(0..num_indices, 0, 0..1);
}
