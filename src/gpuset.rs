use std::collections::HashSet;

// The GpuSet has three states:
//
//  - the set is known to be empty, this is Some({})
//  - the set is known to be nonempty and have only known gpus in the set, this is Some({a,b,..})
//  - the set is known to be nonempty but have (some) unknown members, this is None
//
// During processing, the set starts out as Some({}).  If a device reports "unknown" GPUs then the
// set can transition from Some({}) to None or from Some({a,b,..}) to None.  Once in the None state,
// the set will stay in that state.  There is no representation for some known + some unknown GPUs,
// it is not believed to be worthwhile.

pub type GpuSet = Option<HashSet<usize>>;

pub fn empty_gpuset() -> GpuSet {
    Some(HashSet::new())
}

#[allow(dead_code)]
pub fn gpuset_from_bits(maybe_devices: Option<usize>) -> GpuSet {
    if let Some(mut devs) = maybe_devices {
        let mut gpus = HashSet::new();
        let mut k = 0;
        while devs != 0 {
            if (devs & 1) != 0 {
                gpus.insert(k);
            }
            devs >>= 1;
            k += 1;
        }
        Some(gpus)
    } else {
        None
    }
}

#[allow(dead_code)]
pub fn singleton_gpuset(maybe_device: Option<usize>) -> GpuSet {
    if let Some(dev) = maybe_device {
        let mut gpus = HashSet::new();
        gpus.insert(dev);
        Some(gpus)
    } else {
        None
    }
}

pub fn union_gpuset(lhs: &mut GpuSet, rhs: &GpuSet) {
    if lhs.is_none() {
        // The result is also None
    } else if rhs.is_none() {
        *lhs = None;
    } else {
        lhs.as_mut()
            .expect("LHS is nonempty")
            .extend(rhs.as_ref().expect("RHS is nonempty"));
    }
}
