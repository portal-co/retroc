use super::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct State<V> {
    pub regmap: BTreeMap<V, (Reg, u32)>,
    pub insts: Vec<Inst>,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Inst {
    StoreArg { reg: Reg, fwd: u32 },
    LoadConst { reg: Reg, value: u8 },
    Transfer { from: Reg, to: Reg },
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Op<V> {
    Just(V),
    Const(u8),
}
impl<V> State<V> {
    pub fn add_patch(&mut self, orig: u32, reg: Reg, target: Reg) {
        let l = self.insts.len() as u32 + 1 - orig;
        self.insts
            .insert(orig as usize, Inst::StoreArg { reg, fwd: l });
        self.insts.push(Inst::LoadConst {
            reg: target,
            value: 0u8,
        });
        for i in self.insts[..(orig as usize)].iter_mut() {
            if let Inst::StoreArg { fwd, .. } = i {
                *fwd += 1;
            }
        }
        for m in self.regmap.values_mut() {
            if m.1 >= orig {
                m.1 += 1;
            }
        }
    }
    pub fn sets_at(&self, lim: u32, reg: Reg) -> bool {
        let mut idx = self.insts.len() as u32;
        loop {
            if idx == lim {
                return false;
            }
            idx -= 1;
            match &self.insts[idx as usize] {
                Inst::LoadConst { reg: r, .. } if *r == reg => {
                    return true;
                }
                Inst::Transfer { from, to } if *to == reg && *from != reg => {
                    return true;
                }
                _ => {}
            }
        }
    }
    fn get_into_a(&mut self, this: V)
    where
        V: Clone + core::cmp::Ord,
    {
        if let Some((or, oi)) = self.regmap.get(&this) {
            if self.sets_at(*oi, Reg::A) {
                self.add_patch(*oi, Reg::A, *or);
            } else {
                self.insts.push(Inst::Transfer {
                    from: *or,
                    to: Reg::A,
                });
                self.regmap
                    .insert(this.clone(), (Reg::A, self.insts.len() as u32 - 1));
            }
        }
    }
    pub fn on(&self, this: V, op: Op<V>) -> BTreeSet<State<V>>
    where
        V: Clone + core::cmp::Ord,
    {
        let mut new = self.clone();
        match op {
            Op::Just(v) => {
                if let Some((or, oi)) = new.regmap.get(&v) {
                    if new.sets_at(*oi, *or) {
                        [Reg::A, Reg::X, Reg::Y]
                            .into_iter()
                            .map(|r| {
                                let mut new = new.clone();
                                if new.sets_at(*oi, r) {
                                    new.add_patch(*oi, *or, r);
                                } else {
                                    new.insts.push(Inst::Transfer { from: *or, to: r });
                                }
                                new.regmap
                                    .insert(this.clone(), (r, new.insts.len() as u32 - 1));
                                new
                            })
                            .collect::<BTreeSet<_>>()
                    } else {
                        new.regmap.insert(this.clone(), (*or, *oi));
                        [new].into_iter().collect()
                    }
                } else {
                    [].into_iter().collect()
                }
            }
            Op::Const(a) => [Reg::A, Reg::X, Reg::Y]
                .into_iter()
                .map(|r| {
                    let mut new = new.clone();
                    new.insts.push(Inst::LoadConst { reg: r, value: a });
                    new.regmap
                        .insert(this.clone(), (r, new.insts.len() as u32 - 1));
                    new
                })
                .collect::<BTreeSet<_>>(),
        }
    }
}
