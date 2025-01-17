use super::{Body, Item};
use cubecl_core::{ir::CubeDim, CompilerRepresentation};
use std::fmt::Display;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Binding {
    pub item: Item,
    pub size: Option<usize>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SharedMemory {
    pub index: u16,
    pub item: Item,
    pub size: u32,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LocalArray {
    pub index: u16,
    pub item: Item,
    pub depth: u8,
    pub size: u32,
}

impl LocalArray {
    pub fn new(index: u16, item: Item, depth: u8, size: u32) -> Self {
        Self {
            index,
            item,
            depth,
            size,
        }
    }
}

impl SharedMemory {
    pub fn new(index: u16, item: Item, size: u32) -> Self {
        Self { index, item, size }
    }
}

#[derive(Debug, Clone)]
pub struct ComputeKernel {
    pub inputs: Vec<Binding>,
    pub outputs: Vec<Binding>,
    pub named: Vec<(String, Binding)>,
    pub cube_dim: CubeDim,
    pub body: Body,
    pub wmma_activated: bool,
    pub bf16: bool,
    pub f16: bool,
}

impl CompilerRepresentation for ComputeKernel {
    fn shared_memory_size(&self) -> usize {
        let mut current = 0usize;

        for var in self.body.shared_memories.iter() {
            let factor = match var.item {
                Item::Vec4(_) => 4,
                Item::Vec3(_) => 3,
                Item::Vec2(_) => 2,
                Item::Scalar(_) => 1,
            };

            let elem_size_bytes = var.item.elem().size();
            current += (var.size as usize) * factor * elem_size_bytes;
        }

        current
    }
}

impl Display for ComputeKernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.wmma_activated {
            f.write_str("#include <mma.h>\n")?;
        }
        if self.bf16 {
            f.write_str("#include <cuda_bf16.h>\n")?;
        }

        if self.f16 {
            f.write_str("#include <cuda_fp16.h>\n")?;
        }

        if self.wmma_activated {
            f.write_str("using namespace nvcuda;\n")?;
        }

        if self.f16 {
            f.write_str(
                "
extern \"C\" struct __half4 {
    __half x;
    __half y;
    __half z;
    __half w;
};
",
            )?;
        }

        f.write_fmt(format_args!(
            "
typedef unsigned int uint;

extern \"C\" struct bool4 {{
    bool x;
    bool y;
    bool z;
    bool w;
}};


extern \"C\" __global__ void kernel(
",
        ))?;

        let num_bindings = self.inputs.len() + self.outputs.len() + self.named.len();
        let mut binding_index = 0;
        for (index, binding) in self.inputs.iter().enumerate() {
            binding_index += 1;
            f.write_fmt(format_args!("{} input_{}[]", binding.item, index))?;
            if binding_index < num_bindings {
                f.write_str(",")?;
            }
        }
        for (index, binding) in self.outputs.iter().enumerate() {
            binding_index += 1;
            f.write_fmt(format_args!("{} output_{}[]", binding.item, index))?;
            if binding_index < num_bindings {
                f.write_str(",")?;
            }
        }
        for (name, binding) in self.named.iter() {
            binding_index += 1;
            f.write_fmt(format_args!("{} {}[]", binding.item, name))?;

            if binding_index < num_bindings {
                f.write_str(",")?;
            }
        }

        f.write_str("\n) {\n")?;

        f.write_fmt(format_args!("{}", self.body))?;
        f.write_str("\n}")?;

        Ok(())
    }
}
