use crate::ndarray;
use crate::ndarray_ext;
use crate::ndarray_ext::{NdArray, NdArrayView};
use crate::op;
use crate::ops;
use crate::tensor::Tensor;
use crate::Float;
use std::collections::HashSet;
use std::iter::FromIterator;

pub struct ExpandDims;

pub struct Squeeze;

pub struct Slice {
    pub indices: Vec<ndarray::SliceOrIndex>,
}

pub struct SliceGrad {
    pub indices: Vec<ndarray::SliceOrIndex>,
}

pub struct Split {
    pub axis: isize,
    pub start_index: isize,
    pub end_index: isize,
}

pub struct SplitGrad {
    pub axis: isize,
    pub start_index: isize,
    pub end_index: isize,
}

pub struct Tile {
    pub axis: isize,
    pub num: usize,
}

pub struct Concat {
    pub axis: isize,
}

pub struct ConcatGrad {
    pub axis: isize,
    pub index: usize,
}

pub struct Clip<T: Float> {
    pub min: T,
    pub max: T,
}

pub struct ClipGrad<T: Float> {
    pub min: T,
    pub max: T,
}

pub struct AddN;

pub struct Gather {
    pub axis: isize,
    pub should_normalize_negative_indices: bool,
}

pub struct GatherGrad {
    pub axis: isize,
}

pub struct IndexOp {
    pub index: isize,
}

pub struct IndexOpGrad {
    pub index: isize,
}

pub struct SetDiff1D;

pub struct Shape;

pub struct Rank;

pub struct Size;

pub struct Reshape;

pub struct InferBinOpShape;

impl<T: Float> op::Op<T> for InferBinOpShape {
    fn name(&self) -> &str {
        "InferBinOpShape"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let a_shape_float = &xs[0];
        let b_shape_float = &xs[1];
        let a_shape = a_shape_float.map(|x| x.to_usize().unwrap()).into_raw_vec();
        let b_shape = b_shape_float.map(|x| x.to_usize().unwrap()).into_raw_vec();
        let a_is_scalar = ndarray_ext::is_scalar_shape(a_shape.as_slice());
        let b_is_scalar = ndarray_ext::is_scalar_shape(b_shape.as_slice());

        let ret = if !a_is_scalar && !b_is_scalar {
            let a_rank = a_shape.len();
            let b_rank = b_shape.len();
            assert_eq!(a_rank, b_rank);
            let max = a_shape
                .iter()
                .zip(b_shape)
                .map(|(a, b)| T::from(a.clone().max(b)).unwrap())
                .collect::<Vec<T>>();
            Ok(crate::ArrRepr::Owned(
                NdArray::from_shape_vec(ndarray::IxDyn(&[a_rank]), max).unwrap(),
            ))
        } else if !a_is_scalar {
            Ok(crate::ArrRepr::View(xs[0].clone()))
        } else {
            Ok(crate::ArrRepr::View(xs[1].clone()))
        };
        vec![ret]
    }

    fn grad(&self, _: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![None, None]
    }
}

impl<T: Float> op::Op<T> for Shape {
    fn name(&self) -> &str {
        "Shape"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let x = &xs[0];
        vec![Ok(crate::ArrRepr::Owned(ndarray_ext::shape_of_view(x)))]
    }

    fn grad(&self, _: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![None]
    }
}

impl<T: Float> op::Op<T> for Rank {
    fn name(&self) -> &str {
        "Rank"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let x = &xs[0];
        vec![Ok(crate::ArrRepr::Owned(NdArray::from_elem(
            ndarray::IxDyn(&[]),
            T::from(x.ndim()).unwrap(),
        )))]
    }

    fn grad(&self, _: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![None]
    }
}

impl<T: Float> op::Op<T> for Size {
    fn name(&self) -> &str {
        "Size"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let x = &xs[0];
        vec![Ok(crate::ArrRepr::Owned(NdArray::from_elem(
            ndarray::IxDyn(&[]),
            T::from(x.len()).unwrap(),
        )))]
    }

    fn grad(&self, _: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![None]
    }
}

impl<T: Float> op::Op<T> for Reshape {
    fn name(&self) -> &str {
        "Reshape"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let x = &xs[0];
        let shape_arr = &xs[1];
        let target = shape_arr
            .iter()
            .map(|&dim_size| {
                if dim_size != -T::one() {
                    dim_size.to_usize().unwrap()
                } else {
                    let product: T = shape_arr.iter().fold(T::one(), |acc, &x| acc * x);
                    x.len() / product.neg().to_usize().unwrap()
                }
            })
            .collect::<Vec<_>>();
        // If x is *not* a c-contiguous, just copying it for now
        // due to current state of ndarray: https://github.com/rust-ndarray/ndarray/issues/390
        let ret = if x.is_standard_layout() {
            if let Ok(a) = x.clone().into_shape(ndarray::IxDyn(target.as_slice())) {
                Ok(crate::ArrRepr::View(a))
            } else {
                let copy = crate::ndarray_ext::deep_copy(x);
                if let Ok(a) = copy.into_shape(ndarray::IxDyn(target.as_slice())) {
                    Ok(crate::ArrRepr::Owned(a))
                } else {
                    panic!("Reshape failed: {:?} vs {:?}", x.shape(), target);
                }
            }
        } else if let Ok(a) =
            ndarray_ext::deep_copy(x).into_shape(ndarray::IxDyn(target.as_slice()))
        {
            Ok(crate::ArrRepr::Owned(a))
        } else {
            panic!("Reshape failed: {:?} vs {:?}", x.shape(), target);
        };

        vec![ret]
    }

    fn grad(&self, gy: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        let gx = Tensor::builder()
            .set_inputs(vec![gy, &ops::shape(inputs[0])])
            .build(Reshape);
        vec![Some(gx), None]
    }
}

impl<T: Float> op::Op<T> for SetDiff1D {
    fn name(&self) -> &str {
        "SetDiff1D"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let x0 = &xs[0];
        let x1 = &xs[1];

        let set_a: HashSet<isize> = HashSet::from_iter(
            x0.as_slice()
                .unwrap()
                .iter()
                .map(|&a| a.to_isize().unwrap()),
        );

        let set_b: HashSet<isize> = HashSet::from_iter(
            x1.as_slice()
                .unwrap()
                .iter()
                .map(|&a| a.to_isize().unwrap()),
        );

        let diff = set_a.difference(&set_b);

        let mut vec = diff.collect::<Vec<&isize>>();
        vec.sort();
        let vec = vec
            .into_iter()
            .map(|&a| T::from(a).unwrap())
            .collect::<Vec<T>>();
        let len = vec.len();
        // safe unwrap
        let ret = Ok(crate::ArrRepr::Owned(
            NdArray::from_shape_vec(ndarray::IxDyn(&[len]), vec).unwrap(),
        ));
        vec![ret]
    }

    fn grad(&self, _: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![None, None]
    }
}

impl<T: Float> op::Op<T> for IndexOp {
    fn name(&self) -> &str {
        "IndexOp"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let x = &xs[0];
        let i = if self.index < 0 {
            ((x.len() as isize) + self.index) as usize
        } else {
            self.index as usize
        };
        // unwrap is safe
        let flat_x = x.view().into_shape(x.len()).unwrap();
        let ret = if let Some(ret) = flat_x.get(i) {
            Ok(crate::ArrRepr::Owned(ndarray::arr0(*ret).into_dyn()))
        } else {
            panic!("Index out of bounds");
        };
        vec![ret]
    }

    fn grad(&self, gy: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        let op = IndexOpGrad { index: self.index };
        let gx = Tensor::builder()
            .set_shape(inputs[0].shape())
            .set_inputs(vec![inputs[0], gy])
            .build(op);
        vec![Some(gx)]
    }
}

impl<T: Float> op::Op<T> for IndexOpGrad {
    fn name(&self) -> &str {
        "IndexOpGrad"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let x = &xs[0];
        let gy = &xs[1];
        let mut result = NdArray::zeros(x.shape());
        let i = if self.index < 0 {
            ((x.len() as isize) + self.index) as usize
        } else {
            self.index as usize
        };
        // unwrap is safe
        let len = result.len();
        if let Some(a) = result
            .view_mut()
            .into_shape(len)
            .unwrap() // safe unwrap
            .get_mut(i)
        {
            *a = gy[ndarray::IxDyn(&[])];
        } else {
            panic!("Index out of bounds");
        }
        vec![Ok(crate::ArrRepr::Owned(result))]
    }

    fn grad(&self, _: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![None]
    }
}

impl<T: Float> op::Op<T> for Gather {
    fn name(&self) -> &str {
        "Gather"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let param = &xs[1];
        let indices = &xs[0];
        let indices_shape = indices.shape();
        let param_shape = param.shape();
        let axis = ndarray_ext::normalize_negative_axis(self.axis, param.ndim());

        let output_shape: Vec<usize> = {
            let former: &[usize] = &param_shape[..axis];
            let latter: &[usize] = &param_shape[axis + 1..];
            // doing former + indices.shape() + latter
            former
                .iter()
                .chain(indices_shape)
                .chain(latter)
                .cloned()
                .collect()
        };

        let flat_indices = if self.should_normalize_negative_indices {
            ndarray_ext::normalize_negative_axes(indices, param_shape[axis])
        } else {
            indices.map(|a| a.to_usize().unwrap()).into_raw_vec()
        };
        let selected = param.select(ndarray::Axis(axis), flat_indices.as_slice());
        vec![Ok(crate::ArrRepr::Owned(
            selected.into_shape(output_shape.as_slice()).unwrap(),
        ))]
    }

    fn grad(&self, gy: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        let gx = Tensor::builder()
            .set_shape(inputs[0].shape())
            .set_inputs(vec![inputs[0], inputs[1], gy])
            .build(GatherGrad { axis: self.axis });
        vec![None, Some(gx)]
    }
}

impl<T: Float> op::Op<T> for GatherGrad {
    fn name(&self) -> &str {
        "GatherGrad"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let indices = &xs[0];
        let param = &xs[1];
        let param_shape = param.shape();
        let gy = &xs[2];
        let axis = if self.axis == -1 {
            param.ndim()
        } else {
            self.axis as usize
        };

        // get read-only view of gy and reshape it
        let gy = {
            let former = &param_shape[..axis];
            let latter = &param_shape[axis + 1..];
            let shape: Vec<usize> = former
                .iter()
                .chain(&[indices.len()])
                .chain(latter)
                .cloned()
                .collect();
            gy.view().into_shape(shape).unwrap()
        };

        let mut gx = NdArray::zeros(param.shape());

        for (gy_sub, &i) in gy.axis_iter(ndarray::Axis(axis)).zip(indices) {
            let i = i.to_isize().unwrap();
            // get gx's sub view
            let gx_sliced = gx.slice_mut(
                ndarray::SliceInfo::<_, ndarray::IxDyn>::new(
                    (0..param.ndim())
                        .map(|dim| {
                            if dim == axis {
                                ndarray::SliceOrIndex::Slice {
                                    start: i,
                                    end: Some(i + 1),
                                    step: 1,
                                }
                            } else {
                                ndarray::SliceOrIndex::Slice {
                                    start: 0,
                                    end: None,
                                    step: 1,
                                }
                            }
                        })
                        .collect::<Vec<_>>(),
                )
                .unwrap()
                .as_ref(),
            );

            // squeeze
            let mut gx_sliced = gx_sliced.index_axis_move(ndarray::Axis(axis), 0);
            // assign gy to sliced view
            gx_sliced.zip_mut_with(&gy_sub, |gx, &gy| {
                *gx += gy;
            });
        }

        vec![Ok(crate::ArrRepr::Owned(gx))]
    }

    fn grad(&self, _: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![None, None, None]
    }
}

impl<T: Float> op::Op<T> for AddN {
    fn name(&self) -> &str {
        "AddN"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let ret = if xs.is_empty() {
            unreachable!()
        } else if 1 == xs.len() {
            Ok(crate::ArrRepr::View(xs[0].clone()))
        } else if 2 == xs.len() {
            Ok(crate::ArrRepr::Owned(&xs[0] + &xs[1]))
        } else {
            let mut base = &xs[0] + &xs[1];
            for x in xs[2..].iter() {
                base += &x;
            }
            Ok(crate::ArrRepr::Owned(base))
        };
        vec![ret]
    }

    fn grad(&self, gy: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![Some(gy.clone()); inputs.len()]
    }
}

impl<T: Float> op::Op<T> for Clip<T> {
    fn name(&self) -> &str {
        "Clip"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        vec![Ok(crate::ArrRepr::Owned(
            xs[0].map(move |a| a.min(self.max).max(self.min)),
        ))]
    }

    fn grad(&self, gy: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        let gx = Tensor::builder()
            .set_shape(gy.shape())
            .set_inputs(vec![inputs[0], gy])
            .build(ClipGrad {
                min: self.min,
                max: self.max,
            });
        vec![Some(gx)]
    }
}

impl<T: Float> op::Op<T> for ClipGrad<T> {
    fn name(&self) -> &str {
        "ClipGrad"
    }
    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let mut ret = xs[0].mapv(move |x| {
            // x > min && x < max
            T::from((((x > self.min) as i32) as f32) * (((x < self.max) as i32) as f32)).unwrap()
        });
        ret *= &xs[1];
        vec![Ok(crate::ArrRepr::Owned(ret))]
    }

    fn grad(&self, _: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![None, None]
    }
}

impl<T: Float> op::Op<T> for Concat {
    fn name(&self) -> &str {
        "Concat"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let mut views = vec![];
        let xs = ctx.grab_inputs();
        for x in xs.iter() {
            views.push(x.view());
        }

        let axis = if self.axis < 0 {
            (xs[0].ndim() as isize + self.axis) as usize
        } else {
            self.axis as usize
        };

        let ret = if let Ok(y) = ndarray::stack(ndarray::Axis(axis), views.as_slice()) {
            Ok(crate::ArrRepr::Owned(y))
        } else {
            panic!("Can't concat arrays whose shapes are incompatible.");
        };
        vec![ret]
    }

    fn grad(&self, gy: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        // [x1, x2, x3, ..., gy]
        let mut merged_inputs: Vec<&Tensor<T>> = inputs.to_vec();
        merged_inputs.insert(0, gy);
        let merged_inputs = merged_inputs.as_slice();

        (0..inputs.len())
            .map(move |i| {
                let gx = Tensor::builder()
                    .set_shape(inputs[0].shape())
                    .set_inputs_slice(merged_inputs)
                    .build(ConcatGrad {
                        index: i,
                        axis: self.axis,
                    });
                Some(gx)
            })
            .collect::<Vec<Option<Tensor<T>>>>()
    }
}

impl<T: Float> op::Op<T> for ConcatGrad {
    fn name(&self) -> &str {
        "ConcatGrad"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let gy = &xs[0];
        let xs = xs[1..].to_vec();

        let axis = if self.axis < 0 {
            (xs[0].ndim() as isize + self.axis) as usize
        } else {
            self.axis as usize
        };

        // make slice indices
        let mut start_idx = 0;
        for x in xs[..self.index].iter() {
            start_idx += x.shape()[axis];
        }
        let region_len = xs[self.index].shape()[axis] as isize;
        let indices = (0..gy.ndim())
            .map(move |_axis| {
                if _axis == axis {
                    // partial region
                    ndarray::SliceOrIndex::Slice {
                        start: start_idx as isize,
                        end: Some(region_len),
                        step: 1,
                    }
                } else {
                    // full slice
                    ndarray::SliceOrIndex::Slice {
                        start: 0,
                        end: None,
                        step: 1,
                    }
                }
            })
            .collect::<Vec<_>>();

        // Clone the *view*
        let ret = gy
            .clone()
            .slice_move(ndarray::SliceInfo::new(indices).unwrap().as_ref());
        // do slice
        vec![Ok(crate::ArrRepr::View(ret))]
    }

    fn grad(&self, _: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![None; inputs.len()]
    }
}

impl<T: Float> op::Op<T> for Tile {
    fn name(&self) -> &str {
        "Tile"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let x = &xs[0];
        let axis = ndarray_ext::normalize_negative_axis(self.axis, x.ndim());
        let views = vec![x.clone(); self.num];
        let ret = if let Ok(ret) = ndarray::stack(ndarray::Axis(axis), views.as_slice()) {
            Ok(crate::ArrRepr::Owned(ret))
        } else {
            panic!("Shape Incompatible");
        };
        vec![ret]
    }

    fn grad(&self, gy: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![Some(ops::reduce_sum(gy, &[self.axis], true))]
    }
}

impl<T: Float> op::Op<T> for Split {
    fn name(&self) -> &str {
        "Split"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let x = &ctx.grab_inputs()[0];
        let axis = ndarray_ext::normalize_negative_axis(self.axis, x.ndim());
        let mut ret = x.clone();
        let indices = make_indices_for_split(x, self.start_index, self.end_index, axis);
        ret.slice_collapse(&indices);
        vec![Ok(crate::ArrRepr::View(ret))]
    }

    fn grad(&self, gy: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        let op = SplitGrad {
            axis: self.axis,
            start_index: self.start_index,
            end_index: self.end_index,
        };
        let gx = Tensor::builder()
            .set_inputs(vec![inputs[0], gy])
            .set_shape(inputs[0].shape())
            .build(op);
        vec![Some(gx)]
    }
}

impl<T: Float> op::Op<T> for SplitGrad {
    fn name(&self) -> &str {
        "SplitGrad"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let x = &xs[0];
        let gy = &xs[1];
        let mut gx = NdArray::zeros(x.shape());

        let axis = ndarray_ext::normalize_negative_axis(self.axis, x.ndim());
        let indices = make_indices_for_split(&x, self.start_index, self.end_index, axis);

        gx.slice_mut(
            ndarray::SliceInfo::<_, ndarray::IxDyn>::new(indices)
                .unwrap()
                .as_ref(),
        )
        .zip_mut_with(gy, |a, &g| *a = g);
        vec![Ok(crate::ArrRepr::Owned(gx))]
    }

    fn grad(&self, _: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![None]
    }
}

#[inline]
fn make_indices_for_split<T: Float>(
    x: &NdArrayView<T>,
    start_index: isize,
    end_index: isize,
    axis: usize,
) -> Vec<ndarray::SliceOrIndex> {
    let ndim = x.ndim();
    assert!(ndim > axis, "Wrong split axis");
    (0..ndim)
        .map(|i| {
            if i == axis {
                ndarray::SliceOrIndex::Slice {
                    start: start_index,
                    end: Some(end_index),
                    step: 1,
                }
            } else {
                // full slice
                ndarray::SliceOrIndex::Slice {
                    start: 0,
                    end: None,
                    step: 1,
                }
            }
        })
        .collect::<Vec<_>>()
}

impl<T: Float> op::Op<T> for Slice {
    fn name(&self) -> &str {
        "Slice"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let mut y = ctx.grab_inputs()[0].clone();
        y.slice_collapse(&self.indices);
        vec![Ok(crate::ArrRepr::View(y))]
    }

    fn grad(&self, gy: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        let op = SliceGrad {
            indices: self.indices.clone(),
        };
        let gx = Tensor::builder()
            .set_inputs(vec![inputs[0], gy])
            .set_shape(inputs[0].shape())
            .build(op);
        vec![Some(gx)]
    }
}

impl<T: Float> op::Op<T> for SliceGrad {
    fn name(&self) -> &str {
        "SliceGrad"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let x = &xs[0];
        let gy = &xs[1];
        let mut gx = NdArray::zeros(x.shape());
        // sliced view
        gx.slice_mut(
            ndarray::SliceInfo::<_, ndarray::IxDyn>::new(&self.indices)
                .unwrap()
                .as_ref(),
        )
        .zip_mut_with(&gy, |a, &g| *a = g);
        vec![Ok(crate::ArrRepr::Owned(gx))]
    }

    fn grad(&self, _: &Tensor<T>, _: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        // is this ok?
        vec![None, None]
    }
}
impl<T: Float> op::Op<T> for Squeeze {
    fn name(&self) -> &str {
        "Squeeze"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let mut x = xs[0].clone();
        let mut axes = xs[1]
            .iter()
            .map(|a| a.to_isize().unwrap())
            .collect::<Vec<_>>();
        axes.sort();
        let mut adjust = 0;
        for &i in axes.iter() {
            let axis = if i < 0 {
                (x.ndim() as isize + i as isize) as usize
            } else {
                i as usize
            };
            let axis = axis - adjust;
            assert_eq!(1, x.shape()[axis], "Can't squeeze a dim whose size != 1");
            // axis making ok
            x = x.index_axis_move(ndarray::Axis(axis), 0);
            adjust += 1;
        }
        vec![Ok(crate::ArrRepr::View(x))]
    }

    fn grad(&self, gy: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![Some(ops::expand_dims(gy, inputs[1])), None]
    }
}

impl<T: Float> op::Op<T> for ExpandDims {
    fn name(&self) -> &str {
        "ExpandDims"
    }

    fn compute<'v>(
        &self,
        ctx: crate::runtime::OpComputeContext<'v, T>,
    ) -> op::ComputeResults<'v, T> {
        let xs = ctx.grab_inputs();
        let ret = xs[0].clone();
        let mut axes = xs[1]
            .iter()
            .map(|a| a.to_isize().unwrap())
            .collect::<Vec<_>>();
        axes.sort();
        let mut output_shape = ret.shape().to_vec();
        for &i in axes.iter() {
            let axis = if i < 0 {
                (ret.ndim() as isize + i as isize) as usize
            } else {
                i as usize
            };
            output_shape.insert(axis, 1);
        }
        vec![Ok(crate::ArrRepr::View(
            ret.into_shape(output_shape).unwrap(),
        ))]
    }

    fn grad(&self, gy: &Tensor<T>, inputs: &[&Tensor<T>], _: &Tensor<T>) -> Vec<Option<Tensor<T>>> {
        vec![Some(ops::squeeze(gy, inputs[1])), None]
    }
}
