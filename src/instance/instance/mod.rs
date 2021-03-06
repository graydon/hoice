//! Actual instance structure.

use common::* ;
use instance::info::* ;

pub mod pre_instance ;

pub use self::pre_instance::PreInstance ;

/// A clause.
///
/// Fields are public because a clause is important only if it's in the
/// instance, from which it can only be accessed immutably.
///
/// # Invariants
///
/// - if `! vars[var].active`, then `var` does not appear in `lhs` or `rhs`
#[derive(Clone)]
pub struct Clause {
  /// Variables of the clause.
  pub vars: VarMap<VarInfo>,
  /// Terms of the left-hand side.
  lhs_terms: HConSet<Term>,
  /// Predicate applications of the left-hand side.
  lhs_preds: PredApps,
  /// Single term right-hand side.
  rhs: Option<PredApp>,
}
impl Clause {
  /// Creates a clause.
  pub fn new(
    vars: VarMap<VarInfo>, lhs: Vec<TTerm>, rhs: Option<PredApp>
  ) -> Self {
    let lhs_terms = HConSet::with_capacity( lhs.len() ) ;
    let lhs_preds = PredApps::with_capacity( lhs.len() ) ;
    let mut clause = Clause { vars, lhs_terms, lhs_preds, rhs } ;
    for tterm in lhs { clause.lhs_insert(tterm) ; }
    clause
  }

  /// Checks a clause is well-formed.
  #[cfg(debug_assertions)]
  pub fn check(& self, blah: & 'static str) -> Res<()> {
    use std::iter::Extend ;
    let mut vars = VarSet::with_capacity( self.vars.len() ) ;
    for term in & self.lhs_terms {
      vars.extend( term::vars( term ) )
    }
    for (_, argss) in & self.lhs_preds {
      for args in argss {
        for arg in args {
          vars.extend( term::vars(arg) )
        }
      }
    }
    if let Some((_, ref args)) = self.rhs {
      for arg in args {
        vars.extend( term::vars(arg) )
      }
    }
    for var in vars {
      if ! self.vars[var].active {
        bail!(
          "ill-formed clause: {}, \
          variable {} appears in the clause but is not active",
          blah, self[var]
        )
      }
    }
    Ok(())
  }
  #[cfg(not(debug_assertions))]
  #[inline(always)]
  pub fn check(& self, _: & 'static str) -> Res<()> {
    Ok(())
  }

  /// Deactivates a variable.
  pub fn deactivate(& mut self, var: VarIdx) -> Res<()> {
    debug_assert!( self.vars[var].active ) ;
    self.vars[var].active = false ;
    self.check( "after `deactivate`" )
  }

  /// Inserts a top term in the lhs. Returns true if it was not there.
  #[inline(always)]
  pub fn lhs_insert(& mut self, tterm: TTerm) -> bool {
    match tterm {
      TTerm::T(term) => if let Some(true) = term.bool() {
        false
      } else {
        self.insert_term(term)
      },
      TTerm::P { pred, args } => self.insert_pred_app(pred, args),
    }
  }

  /// Inserts a predicate application in the LHS.
  ///
  /// Returns true if the predicate application is new.
  #[inline(always)]
  pub fn insert_pred_app(
    & mut self, pred: PrdIdx, args: VarMap<Term>
  ) -> bool {
    self.lhs_preds.insert_pred_app(pred, args)
  }

  /// Removes a term from the LHS.
  pub fn rm_term(& mut self, term: & Term) -> bool {
    self.lhs_terms.remove(term)
  }

  /// Inserts a term in an LHS. Externalized for ownership reasons.
  fn lhs_insert_term(lhs_terms: & mut HConSet<Term>, term: Term) -> bool {
    if let Some(kids) = term.conj_inspect() {
      let mut new_stuff = false ;
      let mut stack = vec![] ;
      for kid in kids {
        if let Some(kids) = kid.conj_inspect() {
          for kid in kids { stack.push(kid) }
        } else if let Some(true) = term.bool() {
          ()
        } else {
          let is_new = lhs_terms.insert( kid.clone() ) ;
          new_stuff = new_stuff || is_new
        }
      }
      while let Some(term) = stack.pop() {
        if let Some(kids) = term.conj_inspect() {
          for kid in kids { stack.push(kid) }
        } else if let Some(true) = term.bool() {
          ()
        } else {
          let is_new = lhs_terms.insert( term.clone() ) ;
          new_stuff = new_stuff || is_new
        }
      }
      return new_stuff
    }

    // Only reachable when `term.conj_inspect()` is `None`. Needs to be outside
    // the match because of lexical lifetimes.
    if let Some(true) = term.bool() {
      false
    } else {
      lhs_terms.insert(term)
    }
  }

  /// Inserts a term in the LHS.
  pub fn insert_term(& mut self, term: Term) -> bool {
    Self::lhs_insert_term(& mut self.lhs_terms, term)
  }

  /// Length of a clause's LHS.
  #[inline(always)]
  pub fn lhs_len(& self) -> usize {
    self.lhs_terms.len() + self.lhs_preds.len()
  }
  /// True if the clause's LHS is empty.
  #[inline(always)]
  pub fn lhs_is_empty(& self) -> bool {
    self.lhs_terms.is_empty() && self.lhs_preds.is_empty()
  }

  /// LHS accessor (terms).
  #[inline(always)]
  pub fn lhs_terms(& self) -> & HConSet<Term> {
    & self.lhs_terms
  }
  /// LHS accessor (predicate applications).
  #[inline(always)]
  pub fn lhs_preds(& self) -> & PredApps {
    & self.lhs_preds
  }
  /// Number of predicate applications in the lhs (>= number of predicates).
  pub fn lhs_pred_apps_len(& self) -> usize {
    let mut sum = 0 ;
    for argss in self.lhs_preds.values() {
      sum += argss.len()
    }
    sum
  }
  /// RHS accessor.
  #[inline(always)]
  pub fn rhs(& self) -> Option<(PrdIdx, & VarMap<Term>)> {
    if let Some((prd, ref args)) = self.rhs {
      Some((prd, args))
    } else {
      None
    }
  }

  /// Variables accessor.
  #[inline(always)]
  pub fn vars(& self) -> & VarMap<VarInfo> {
    & self.vars
  }

  /// Clones a clause but changes the rhs.
  #[inline(always)]
  pub fn clone_with_rhs(& self, rhs: TTerm) -> Self {
    let mut lhs_terms = self.lhs_terms.clone() ;
    let rhs = match rhs {
      TTerm::P { pred, args } => Some((pred, args)),
      TTerm::T(term) => {
        if term.bool() != Some(false) {
          lhs_terms.insert( term::not(term) ) ;
        }
        None
      },
    } ;
    Clause {
      vars: self.vars.clone(),
      lhs_terms,
      lhs_preds: self.lhs_preds.clone(),
      rhs,
    }
  }



  /// Variable substitution.
  ///
  /// Returns a boolean indicating whether any substitution occured.
  ///
  /// Used for substitutions in the same clause / predicate scope.
  pub fn subst<Map: VarIndexed<Term>>(
    & mut self, map: & Map
  ) -> bool {
    let mut changed = false ;
    let mut lhs_terms = HConSet::with_capacity( self.lhs_terms.len() ) ;
    ::std::mem::swap(& mut lhs_terms, & mut self.lhs_terms) ;
    for term in lhs_terms.drain() {
      let (term, b) = term.subst(map) ;
      self.insert_term(term) ;
      changed = changed || b
    }
    for (_, argss) in & mut self.lhs_preds {
      let mut nu_argss = Vec::with_capacity( argss.len() ) ;
      debug_assert!( nu_argss.is_empty() ) ;
      for mut args in argss.drain(0..) {
        for arg in args.iter_mut() {
          let (nu_arg, b) = arg.subst(map) ;
          * arg = nu_arg ;
          changed = changed || b
        }
        nu_argss.push(args) ;
      }
      ::std::mem::swap(& mut nu_argss, argss)
    }
    if let Some(& mut (_, ref mut args)) = self.rhs.as_mut() {
      for arg in args.iter_mut() {
        let (nu_arg, b) = arg.subst(map) ;
        * arg = nu_arg ;
        changed = changed || b
      }
    }
    changed
  }

  /// Adds fresh variables to the clause for each of the input variables.
  /// Returns a map from the input variables to the fresh ones (as terms).
  ///
  /// Used when inlining a predicate with quantified variables.
  fn fresh_vars_for(& mut self, vars: & Quantfed) -> VarHMap<Term> {
    let mut map = VarHMap::with_capacity( vars.len() ) ;
    for (var, typ) in vars {
      let fresh = self.vars.next_index() ;
      let fresh_name = format!("hoice_fresh_var@{}", fresh) ;
      let info = VarInfo::new(fresh_name, * typ, fresh) ;
      self.vars.push(info) ;
      let _prev = map.insert(* var, term::var(fresh)) ;
      debug_assert!( _prev.is_none() )
    }
    map
  }

  /// Adds fresh variables to the clause for each of the input variables.
  /// Returns a map from the input variables to the fresh ones (as terms).
  ///
  /// Used when inlining a predicate with quantified variables.
  fn nu_fresh_vars_for(& mut self, quant: & Option<Quant>) -> VarHMap<Term> {
    if let Some(quant) = quant.as_ref() {
      let vars = quant.vars() ;
      let mut map = VarHMap::with_capacity( vars.len() ) ;
      for (var, typ) in vars {
        let fresh = self.vars.next_index() ;
        let fresh_name = format!("hoice_fresh_var@{}", fresh) ;
        let info = VarInfo::new(fresh_name, * typ, fresh) ;
        self.vars.push(info) ;
        let _prev = map.insert(* var, term::var(fresh)) ;
        debug_assert!( _prev.is_none() )
      }
      map
    } else {
      return VarHMap::new()
    }
  }

  // /// Replaces a predicate application by some top terms.
  // ///
  // /// Does not preserve the order of the top terms.
  // pub fn subst_top(& mut self, pred: PrdIdx, top) -> 

  /// Writes a clause given a special function to write predicates.  
  fn write<W, WritePrd>(
    & self, w: & mut W, write_prd: WritePrd
  ) -> IoRes<()>
  where W: Write, WritePrd: Fn(& mut W, PrdIdx, & VarMap<Term>) -> IoRes<()> {
    use common::consts::keywords ;

    write!(w, "({} ({}\n  (", keywords::assert, keywords::forall) ? ;
    let mut inactive = 0 ;
    for var in & self.vars {
      if var.active {
        write!(w, " ({} {})", var.name, var.typ) ?
      } else {
        inactive += 1 ;
      }
    }
    write!(w, " )") ? ;
    if inactive > 0 {
      write!(w, " ; {} inactive variable(s)", inactive) ?
    }
    write!(w, "\n") ? ;

    let lhs_len = self.lhs_len() ;

    let (pref, suff) = if lhs_len != 0 {
      write!(w, "  (=>") ? ;
      let (pref, suff) = if lhs_len > 1 {
        write!(w, "\n    (and") ? ;
        ("      ", Some("    )"))
      } else {
        ("    ", None)
      } ;

      for term in & self.lhs_terms {
        write!(w, "\n{}", pref) ? ;
        term.write(w, |w, var| w.write_all( self.vars[var].as_bytes() )) ?
      }
      for (pred, argss) in & self.lhs_preds {
        for args in argss {
          write!(w, "\n{}", pref) ? ;
          write_prd(w, * pred, args) ?
        }
      }

      write!(w, "\n") ? ;
      if let Some(suff) = suff {
        write!(w, "{}\n", suff) ?
      }
      ("    ", Some("  )"))
    } else {
      ("  ", None)
    } ;

    write!(w, "{}", pref) ? ;
    if let Some((pred, ref args)) = self.rhs {
      write_prd(w, pred, args) ?
    } else {
      write!(w, "false") ?
    }
    write!(w, "\n") ? ;
    if let Some(suff) = suff {
      write!(w, "{}\n", suff) ?
    }
    write!(w, "))")
  }
}
impl ::std::ops::Index<VarIdx> for Clause {
  type Output = VarInfo ;
  fn index(& self, index: VarIdx) -> & VarInfo {
    & self.vars[index]
  }
}
impl<'a, 'b> ::rsmt2::to_smt::Expr2Smt<
  & 'b (& 'a PrdSet, & 'a PrdSet, & 'a PrdMap<PrdInfo>)
> for Clause {
  fn expr_to_smt2<Writer: Write>(
    & self, writer: & mut Writer, info: & 'b (
      & 'a PrdSet, & 'a PrdSet, & 'a PrdMap<PrdInfo>
    )
  ) -> SmtRes<()> {
    let (ref true_preds, ref false_preds, ref prd_info) = * info ;
    write!(writer, "(not ") ? ;
    if ! self.lhs_is_empty() {
      write!(writer, "(=> (and") ?
    }
    for term in & self.lhs_terms {
      writer.write_all( " ".as_bytes() ) ? ;
      term.write( writer, |w, var| var.default_write(w) ) ?
    }
    for (pred, argss) in & self.lhs_preds {
      if true_preds.contains(pred) {
        writer.write_all( " true".as_bytes() ) ?
      } else if false_preds.contains(pred) {
        writer.write_all( " false".as_bytes() ) ?
      } else {
        for args in argss {
          writer.write_all( " (".as_bytes() ) ? ;
          writer.write_all( prd_info[* pred].name.as_bytes() ) ? ;
          for arg in args {
            writer.write_all( " ".as_bytes() ) ? ;
            arg.write(writer, |w, var| var.default_write(w)) ?
          }
          writer.write_all( ")".as_bytes() ) ?
        }
      }
    }
    if ! self.lhs_is_empty() {
      write!(writer, ") ") ?
    }
    if let Some((prd, ref args)) = self.rhs {
      if true_preds.contains(& prd) {
        write!(writer, "true") ?
      } else if false_preds.contains(& prd) {
        write!(writer, "false") ?
      } else {
        write!(writer, "({}", prd_info[prd].name) ? ;
        for arg in args {
          write!(writer, " ") ? ;
          arg.write(writer, |w, var| var.default_write(w)) ?
        }
        write!(writer, ")") ?
      }
    } else {
      write!(writer, "false") ?
    }
    if ! self.lhs_is_empty() {
      write!(writer, ")") ?
    }
    write!(writer, ")") ? ;
    Ok(())
  }
}




/// Stores the instance: the clauses, the factory and so on.
///
/// # NB
///
/// Clause indices can vary during instance building, because of the
/// simplifications that can remove clauses.
///
/// So, `pred_to_clauses` has to be carefully maintained, the easiest way to
/// do this is to never access an instance's fields directly from the outside.
pub struct Instance {
  /// Constants constructed so far.
  consts: HConSet<Term>,
  /// Predicates.
  preds: PrdMap<PrdInfo>,
  /// Original predicates, for reconstruction.
  ///
  /// Stores the original signature of the predicates, and a map from the
  /// variables of `preds` to the original signature.
  old_preds: PrdMap< (VarMap<Typ>, VarMap<VarIdx>) >,
  /// Predicates for which a suitable term has been found.
  pred_terms: PrdMap< Option< TTerms > >,
  /// Predicates defined in `pred_terms`, sorted by predicate dependencies.
  ///
  /// Populated by the `finalize` function.
  sorted_pred_terms: Vec<PrdIdx>,
  /// Max arity of the predicates.
  pub max_pred_arity: Arity,
  /// Clauses.
  clauses: ClsMap<Clause>,
  /// Maps predicates to the clauses where they appear in the lhs and rhs
  /// respectively.
  pred_to_clauses: PrdMap< (ClsSet, ClsSet) >,
  /// Unsat flag.
  is_unsat: bool,
}
impl Instance {
  /// Instance constructor.
  pub fn new() -> Instance {
    let pred_capa = conf.instance.pred_capa ;
    let clause_capa = conf.instance.clause_capa ;
    let mut instance = Instance {
      consts: HConSet::with_capacity(103),
      preds: PrdMap::with_capacity(pred_capa),
      old_preds: PrdMap::with_capacity(pred_capa),
      pred_terms: PrdMap::with_capacity(pred_capa),
      sorted_pred_terms: Vec::with_capacity(pred_capa),
      max_pred_arity: 0.into(),
      clauses: ClsMap::with_capacity(clause_capa),
      // clusters: CtrMap::with_capacity( clause_capa / 3 ),
      pred_to_clauses: PrdMap::with_capacity(pred_capa),
      is_unsat: false,
    } ;
    // Create basic constants, adding to consts to have mining take them into account.
    let (wan,too) = (term::one(), term::zero()) ;
    instance.consts.insert(wan) ;
    instance.consts.insert(too) ;
    instance
  }

  /// Number of active (not forced) predicates.
  pub fn active_pred_count(& self) -> usize {
    let mut count = 0 ;
    for pred in self.pred_indices() {
      if ! self.is_known(pred) { count += 1 }
    }
    count
  }

  /// Returns true if the instance is already solved.
  pub fn is_solved(& self) -> bool {
    if self.is_unsat { return true }
    for def in & self.pred_terms {
      if def.is_none() { return false }
    }
    true
  }

  /// Sets the unsat flag in the instance.
  pub fn set_unsat(& mut self) {
    self.is_unsat = true
  }

  /// True if a predicate is forced to something.
  #[inline]
  pub fn is_known(& self, pred: PrdIdx) -> bool {
    self.pred_terms[pred].is_some()
  }

  /// Returns the model corresponding to the input predicates and the forced
  /// predicates.
  ///
  /// The model is sorted in topological order.
  pub fn model_of(& self, candidates: Candidates) -> Res<Model> {
    use std::iter::Extend ;
    let mut model = Model::with_capacity( self.preds.len() ) ;
    model.extend(
      candidates.into_index_iter().filter_map(
        |(pred, tterms_opt)| tterms_opt.map(
          |term| (pred, TTerms::of_term(None, term))
        )
      )
    ) ;
    for pred in & self.sorted_pred_terms {
      let pred = * pred ;
      if let Some(ref tterms) = self.pred_terms[pred] {
        model.push(
          (pred, tterms.clone())
        )
      } else {
        bail!("inconsistency in sorted forced predicates")
      }
    }
    Ok( model )
  }

  /// Returns a model for the instance when all the predicates have terms
  /// assigned to them.
  pub fn is_trivial(& self) -> Res< Option< Option<Model> > > {
    if self.is_unsat { Ok( Some(None) ) } else {
      for pred in self.pred_indices() {
        if self.pred_terms[pred].is_none() {
          return Ok(None)
        }
      }
      // Only reachable if all elements of `self.pred_terms` are `Some(_)`.
      self.model_of( vec![].into() ).map(|res| Some(Some(res)))
    }
  }


  /// Clauses a predicate appears in. Lhs and rhs.
  pub fn clauses_of_pred(& self, pred: PrdIdx) -> ( & ClsSet, & ClsSet ) {
    ( & self.pred_to_clauses[pred].0, & self.pred_to_clauses[pred].1 )
  }
  /// Lhs and rhs predicates of a clause.
  #[inline]
  pub fn preds_of_clause(
    & self, clause: ClsIdx
  ) -> (& PredApps, Option<PrdIdx>) {
    (
      self[clause].lhs_preds(), self[clause].rhs().map(|(prd, _)| prd)
    )
  }


  /// Prints some top terms as a model.
  ///
  /// Meaning variables are printed with default printing: `<var_idx>` is
  /// printed as `v_<var_idx>`.
  pub fn print_tterms_as_model<W: Write>(
    & self, w: & mut W, tterms: & TTerms
  ) -> IoRes<()> {
    tterms.write(
      w, |w, var| var.default_write(w),
      |w, pred, args| {
        write!(w, "({}", self[pred]) ? ;
        let mut prev: VarIdx = 0.into() ;
        for (var, arg) in args.index_iter() {
          let old_var = self.old_preds[pred].1[var] ;
          for var in VarRange::new(prev, old_var) {
            write!(
              w, " {}", self.old_preds[pred].0[var].default_val()
            ) ?
          }
          prev = old_var ;
          prev.inc() ;
          write!(w, " {}", arg) ?
        }
        for var in VarRange::new(prev, self.old_preds[pred].0.next_index()) {
          write!(
            w, " {}", self.old_preds[pred].0[var].default_val()
          ) ?
        }
        write!(w, ")")
      }
    )
  }

  /// Finalizes instance creation.
  ///
  /// - shrinks all collections
  /// - sorts forced predicates by dependencies
  ///
  /// # TO DO
  ///
  /// - optimize sorting of forced preds by dependencies (low priority)
  pub fn finalize(& mut self) {
    self.sorted_pred_terms.clear() ;
    self.consts.shrink_to_fit() ;
    self.preds.shrink_to_fit() ;
    self.old_preds.shrink_to_fit() ;
    self.pred_terms.shrink_to_fit() ;
    self.clauses.shrink_to_fit() ;

    let mut tmp: Vec< (PrdIdx, PrdSet) > = Vec::with_capacity(
      self.preds.len()
    ) ;

    // Populate `tmp`.
    let mut known_preds = PrdSet::with_capacity( self.preds.len() ) ;
    self.max_pred_arity = 0.into() ;
    for pred in self.pred_indices() {
      if let Some(ref tterms) = self.pred_terms[pred] {
        tmp.push( (pred, tterms.preds()) )
      } else {
        self.max_pred_arity = ::std::cmp::max(
          self.max_pred_arity, (self[pred].sig.len() + 1).into()
        ) ;
        known_preds.insert(pred) ;
      }
    }
    // Sort by dependencies.
    while ! tmp.is_empty() {
      let mut cnt = 0 ; // Will use swap remove.
      'find_preds: while cnt < tmp.len() {
        for pred in & tmp[cnt].1 {
          if ! known_preds.contains(pred) {
            // Don't know this predicate, keep going in `tmp`.
            cnt += 1 ;
            continue 'find_preds
          }
        }
        // Reachable only we already have all of the current pred's
        // dependencies.
        let (pred, _) = tmp.swap_remove(cnt) ;
        self.sorted_pred_terms.push(pred) ;
        known_preds.insert(pred) ;
        () // No `cnt` increment after swap remove.
      }
    }

    self.sorted_pred_terms.shrink_to_fit() ;

    // If there are no clusters just create one cluster per clause.
    // if self.clusters.is_empty() {
    //   log_info! { "instance has no clusters, creating single clause clusters" }
    //   for (idx, clause) in self.clauses.index_iter() {
    //     self.clusters.push( Cluster::of_clause(idx, clause) )
    //   }
    // }
  }


  /// Returns the term we already know works for a predicate, if any.
  pub fn forced_terms_of(& self, pred: PrdIdx) -> Option<& TTerms> {
    self.pred_terms[pred].as_ref()
  }

  /// If the input predicate is forced to a constant boolean, returns its
  /// value.
  pub fn bool_value_of(& self, pred: PrdIdx) -> Option<bool> {
    self.forced_terms_of(pred).and_then(
      |terms| terms.bool()
    )
  }

  /// Forced predicates in topological order.
  pub fn sorted_forced_terms(& self) -> & Vec<PrdIdx> {
    & self.sorted_pred_terms
  }

  /// Returns the clauses in which the predicate appears in the lhs and rhs
  /// respectively.
  pub fn clauses_of(& self, pred: PrdIdx) -> (& ClsSet, & ClsSet) {
    (& self.pred_to_clauses[pred].0, & self.pred_to_clauses[pred].1)
  }

  /// Adds a predicate application to a clause's lhs.
  pub fn clause_add_lhs_pred(
    & mut self, clause: ClsIdx, pred: PrdIdx, args: VarMap<Term>
  ) {
    self.clauses[clause].insert_pred_app(pred, args) ;
    self.pred_to_clauses[pred].0.insert(clause) ;
  }

  /// Adds a term to a clause's lhs.
  pub fn clause_add_lhs_term(
    & mut self, clause: ClsIdx, term: Term
  ) {
    self.clauses[clause].insert_term(term) ;
  }

  /// Forces the rhs of a clause to a predicate application.
  pub fn clause_force_rhs(
    & mut self, clause: ClsIdx, pred: PrdIdx, args: VarMap<Term>
  ) {
    self.pred_to_clauses[pred].1.insert(clause) ;
    self.clauses[clause].rhs = Some((pred, args))
  }

  /// Adds some terms to the lhs of a clause.
  ///
  /// Updates `pred_to_clauses`.
  pub fn clause_lhs_extend<I: IntoIterator<Item = TTerm>>(
    & mut self, clause_idx: ClsIdx, tterms: I
  ) {
    let clause = & mut self.clauses[clause_idx] ;
    for tterm in tterms.into_iter() {
      match tterm {
        TTerm::P { pred, args } => {
          self.pred_to_clauses[pred].0.insert(clause_idx) ;
          clause.insert_pred_app(pred, args) ;
        },
        TTerm::T(term) => {
          clause.insert_term(term) ;
        },
      }
    }
  }

  /// Replaces the rhs of a clause.
  ///
  /// Updates `pred_to_clauses` for the term it inserts but **not** the one it
  /// removes.
  pub fn clause_rhs_force(
    & mut self, clause_idx: ClsIdx, tterm: TTerm
  ) {
    let clause = & mut self.clauses[clause_idx] ;
    match tterm {
      TTerm::P { pred, args } => {
        clause.rhs = Some((pred, args)) ;
        let is_new = self.pred_to_clauses[pred].1.insert(clause_idx) ;
        debug_assert!( is_new )
      },
      TTerm::T(term) => {
        if term.bool() != Some(false) {
          clause.lhs_terms.insert( term::not(term) ) ;
        }
        clause.rhs = None
      },
    }
  }

  // /// Evaluates the term a predicate is forced to, if any.
  // pub fn eval_term_of(
  //   & self, pred: PrdIdx, model: & VarMap<Val>
  // ) -> Res< Option<bool> > {
  //   if let Some(term) = self.term_of(pred) {
  //     match term.bool_eval(model) {
  //       Ok(None) => bail!("partial model during predicate term evaluation"),
  //       res => res,
  //     }
  //   } else {
  //     Ok(None)
  //   }
  // }

  /// Set of int constants **appearing in the predicates**. If more constants
  /// are created after the instance building step, they will not appear here.
  pub fn consts(& self) -> & HConSet<Term> {
    & self.consts
  }

  /// Range over the predicate indices.
  pub fn pred_indices(& self) -> PrdRange {
    PrdRange::zero_to( self.preds.len() )
  }
  /// Range over the clause indices.
  pub fn clause_indices(& self) -> ClsRange {
    ClsRange::zero_to( self.clauses.len() )
  }

  /// Predicate accessor.
  pub fn preds(& self) -> & PrdMap<PrdInfo> {
    & self.preds
  }
  /// Clause accessor.
  pub fn clauses(& self) -> & ClsMap<Clause> {
    & self.clauses
  }

  /// Removes all predicate applications of some predicate in the lhs of a
  /// clause.
  fn rm_pred_apps_in_lhs(& mut self, pred: PrdIdx, clause: ClsIdx) {
    self.pred_to_clauses[pred].0.remove(& clause) ;
    self.clauses[clause].lhs_preds.remove(& pred) ;
  }


  /// Strengthens some predicate by some terms using the clauses lhs where the
  /// predicate appears.
  ///
  /// Returns the number of clauses created.
  ///
  /// Currently pseudo-inactive. Can only remove predicate applications if they
  /// are found to be trivial given the strengthening.
  ///
  /// For all clauses `c` where `pred` appears in the lhs, creates a new clause
  /// that is `c` with every application of `pred` replaced by `tterms`
  /// instantiated on `pred`'s application arguments.
  pub fn strengthen_in_lhs(
    & mut self, pred: PrdIdx, tterms: Vec<TTerm>
  ) -> Res<usize> {
    // let mut nu_clauses = Vec::with_capacity(
    //   self.pred_to_clauses[pred].0.len()
    // ) ;
    let mut nu_tterms = HashSet::with_capacity( 29 ) ;
    let mut pred_apps_to_rm = Vec::with_capacity(11) ;

    'clause_iter: for clause in & self.pred_to_clauses[pred].0 {
      // debug_assert!( nu_tterms.is_empty() ) ;
      nu_tterms.clear() ;

      log_debug!{ "  - #{}", clause }

      if let Some(argss) = self[* clause].lhs_preds.get(& pred) {

        log_debug!{ "    {} applications", argss.len() }
        for args in argss {
          'tterm_iter: for tterm in & tterms {
            let tterm = tterm.subst_total(args) ? ;
            if let Some(b) = tterm.bool() {
              if ! b {
                log_debug!{ "      new clause is trivial, skipping" }
                continue 'clause_iter
              }
            } else {
              match tterm {
                TTerm::T(ref term) if self[
                  * clause
                ].lhs_terms.contains(term) => continue 'tterm_iter,
                TTerm::P { ref pred, ref args } if self[
                  * clause
                ].lhs_preds.get(pred).map(
                  |argss| argss.contains(args)
                ).unwrap_or(false) => continue 'tterm_iter,
                _ => ()
              }
              log_debug!{ "    - {}", tterm }
              nu_tterms.insert( tterm ) ;
            }
          }
        }

      } else {
        bail!(
          "inconsistent instance state \
          (`pred_to_clauses` in `strengthen_in_lhs`)"
        )
      }

      if nu_tterms.is_empty() {
        log_debug!{
          "    no new terms, can remove applications of this predicate"
        }
        pred_apps_to_rm.push( (pred, * clause) )
      } else {
        // let mut nu_clause = self[* clause].clone() ;

        // for tterm in nu_tterms.drain() {
        //   nu_clause.lhs_insert(tterm) ;
        // }

        // let should_remove = self.simplifier.clause_propagate(
        //   & mut nu_clause
        // ) ? ;
        // if should_remove {
        //   log_info!{
        //     "    new clause is trivial after propagation"
        //   }
        // } else {
        //   nu_clauses.push(nu_clause)
        // }
      }

    }

    for (pred, clause) in pred_apps_to_rm {
      self.rm_pred_apps_in_lhs(pred, clause)
    }
    // let count = nu_clauses.len() ;
    // log_info!{ "    adding {} clauses", count }
    // for clause in nu_clauses { self.push_clause(clause) ? }
    self.check("after strengthening (lhs)") ? ;

    // Ok(count)
    Ok(0)
  }

  /// Pushes a new predicate and returns its index.
  pub fn push_pred(
    & mut self, name: String, sig: VarMap<Typ>
  ) -> PrdIdx {
    self.max_pred_arity = ::std::cmp::max(
      self.max_pred_arity, (sig.len() + 1).into()
    ) ;
    let idx = self.preds.next_index() ;
    let mut var_map = VarMap::with_capacity( sig.len() ) ;
    for (var, _) in sig.index_iter() {
      var_map.push(var)
    }
    self.old_preds.push(
      (sig.clone(), var_map)
    ) ;
    self.preds.push( PrdInfo {
      name, idx, sig
    } ) ;
    self.pred_terms.push(None) ;

    self.pred_to_clauses.push(
      ( ClsSet::with_capacity(17), ClsSet::with_capacity(17) )
    ) ;
    idx
  }

  /// Removes and returns the indices of the clauses `pred` appears in the lhs
  /// of from `self.pred_to_clauses`.
  fn unlink_pred_lhs<LHS>(& mut self, pred: PrdIdx, lhs: & mut LHS)
  where LHS: ::std::iter::Extend<ClsIdx> {
    lhs.extend( self.pred_to_clauses[pred].0.drain() )
  }

  /// Removes and returns the indices of the clauses `pred` appears in the rhs
  /// of from `self.pred_to_clauses`.
  fn unlink_pred_rhs<RHS>(& mut self, pred: PrdIdx, rhs: & mut RHS)
  where RHS: ::std::iter::Extend<ClsIdx> {
    rhs.extend( self.pred_to_clauses[pred].1.drain() )
  }

  /// Goes trough the predicates in `from`, and updates `pred_to_clauses` so
  /// that they point to `to` instead.
  fn relink_preds_to_clauses(
    & mut self, from: ClsIdx, to: ClsIdx
  ) -> Res<()> {
    for pred in self.clauses[from].lhs_preds.keys() {
      let pred = * pred ;
      let was_there = self.pred_to_clauses[pred].0.remove(& from) ;
      let _ = self.pred_to_clauses[pred].0.insert(to) ;
      debug_assert!(was_there)
    }
    if let Some((pred, _)) = self.clauses[from].rhs() {
      let was_there = self.pred_to_clauses[pred].1.remove(& from) ;
      let _ = self.pred_to_clauses[pred].1.insert(to) ;
      debug_assert!(was_there)
    }
    Ok(())
  }

  // /// Unlinks a predicate from a clause.
  // fn unlink_pred_and_clause(
  //   & mut self, pred: PrdIdx, clause: ClsIdx
  // ) -> Res<()> {
  //   let in_lhs = self.pred_to_clauses[pred].0.remove(& clause) ;
  //   let in_rhs = self.pred_to_clauses[pred].1.remove(& clause) ;
  //   if ! (in_lhs && in_rhs ) {
  //     bail!(
  //       "predicate {} is not linked to clause number {}, failed to unlink",
  //       conf.sad(& self[pred].name), clause
  //     )
  //   } else {
  //     Ok(())
  //   }
  // }

  /// Forget some clauses.
  ///
  /// Duplicates are handled as if there was only one.
  pub fn forget_clauses(
    & mut self, clauses: & mut Vec<ClsIdx>
  ) -> Res<()> {
    // Forgetting is done by swap remove, so we sort in DESCENDING order so
    // that indices always make sense.
    clauses.sort_unstable_by(
      |c_1, c_2| c_2.cmp(c_1)
    ) ;
    let mut prev = None ;
    for clause in clauses.drain(0..) {
      log_debug!{
        "    forgetting {}", self[clause].to_string_info(& self.preds) ?
      }
      if prev == Some(clause) { continue }
      prev = Some(clause) ;
      let _ = self.forget_clause(clause) ? ;
    }
    // self.check("after `forget_clause`") ? ;
    Ok(())
  }

  /// Forget a clause. **Does not preserve the order of the clauses.**
  ///
  /// After calling this function, clause indices kept outside of the instance
  /// will in general refer to clauses different from the ones they pointed to
  /// before the call.
  ///
  /// Also unlinks predicates from `pred_to_clauses`.
  pub fn forget_clause(& mut self, clause: ClsIdx) -> Res<Clause> {
    for pred in self.clauses[clause].lhs_preds.keys() {
      let pred = * pred ;
      let was_there = self.pred_to_clauses[pred].0.remove(& clause) ;
      debug_assert!(
        was_there || self.is_known(pred)
      )
    }
    if let Some((pred, _)) = self.clauses[clause].rhs() {
      let was_there = self.pred_to_clauses[pred].1.remove(& clause) ;
      debug_assert!(
        was_there || self.is_known(pred)
      )
    }
    // Relink the last clause as its index is going to be `clause`. Except if
    // `clause` is already the last one.
    let last_clause: ClsIdx = ( self.clauses.len() - 1 ).into() ;
    if clause != last_clause {
      self.relink_preds_to_clauses(last_clause, clause) ?
    }
    let res = self.clauses.swap_remove(clause) ;
    Ok(res)
  }

  /// Pushes a new clause.
  pub fn push_clause(& mut self, clause: Clause) -> Res<()> {
    self.push_clause_unchecked(clause) ;
    self.check("after `push_clause`")
  }

  /// Pushes a new clause, does not sanity-check.
  fn push_clause_unchecked(& mut self, clause: Clause) -> () {
    let clause_index = self.clauses.next_index() ;
    for pred in clause.lhs_preds.keys() {
      let pred = * pred ;
      let is_new = self.pred_to_clauses[pred].0.insert(clause_index) ;
      debug_assert!(is_new)
    }
    if let Some((pred, _)) = clause.rhs() {
      let is_new = self.pred_to_clauses[pred].1.insert(clause_index) ;
      debug_assert!(is_new)
    }
    self.clauses.push(clause)
  }

  /// Extracts some qualifiers from all clauses.
  pub fn qualifiers(& self, quals: & mut Quals) {
    for clause in & self.clauses {
      // println!(
      //   "  - mining clause\n{}",
      //   clause.to_string_info(& self.preds).unwrap()
      // ) ;
      self.qualifiers_of_clause(clause, quals)
    }
  }

  /// Extracts some qualifiers from a clause.
  ///
  /// # TO DO
  ///
  /// - write an explanation of what actually happens
  /// - and some tests, probably
  pub fn qualifiers_of_clause(
    & self, clause: & Clause, quals: & mut Quals
  ) {

    // println!(
    //   "qualifiers for clause {}",
    //   clause.to_string_info(& self.preds).unwrap()
    // ) ;

    // Extraction of the variables map based on the way the predicates are
    // used.
    let mut maps = vec![] ;

    // Qualifiers generated while looking at predicate applications.
    let mut app_quals: HConSet<Term> = HConSet::with_capacity(17) ;

    let rhs_opt = if let Some((ref pred, ref args)) = clause.rhs {
      let mut set = Vec::with_capacity(1) ;
      set.push(args.clone()) ;
      Some((pred, set))
    } else { None } ;

    {
      // Represents equalities between *pred vars* and terms over *clause
      // variables*. These will be added to `app_quals` if the total
      // substitution of the term by `map` succeeds.
      let mut eq_quals = VarHMap::with_capacity(7) ;

      let rhs_opt = rhs_opt.as_ref().map( |& (pred, ref set)| (pred, set) ) ;

      for (_, argss) in clause.lhs_preds.iter().chain( rhs_opt.into_iter() ) {
        debug_assert!( app_quals.is_empty() ) ;
        for args in argss {
          debug_assert!( eq_quals.is_empty() ) ;

          // All the *clause var* to *pred var* maps for this predicate
          // application.
          let mut map: VarHMap<Term> = VarHMap::with_capacity( args.len() ) ;

          // println!("  iterating over pred app") ;
          for (pred_var, term) in args.index_iter() {
            // println!("v_{}: {}", pred_var, term) ;

            // Parameter's a variable?
            if let Some(clause_var_index) = term.var_idx() {

              // Clause variable's already known as parameter?
              if let Some(other_pred_var) = map.get(& clause_var_index).map(
                |t| t.clone()
              ) {
                // Equality qualifier.
                app_quals.insert(
                  term::eq( term::var(pred_var), other_pred_var.clone() )
                ) ;
              } else {
                // Add to map.
                let _prev = map.insert(clause_var_index, term::var(pred_var)) ;
                debug_assert!( _prev.is_none() )
              }

            } else {
              // Parameter's not a variable, store potential equality.
              let _prev = eq_quals.insert(pred_var, term) ;
              debug_assert!( _prev.is_none() ) ;
              // Try to revert the term.
              if let Some((var, term)) = term.invert(pred_var) {
                if ! map.contains_key(& var) {
                  map.insert(var, term) ;
                }
              }
            }

          }

          // println!("  generating var / term equalities") ;
          for (pred, term) in eq_quals.drain() {
            if let Some((term, _)) = term.subst_total(& map) {
              app_quals.insert( term::eq( term::var(pred), term ) ) ;
            }
          }

          if ! app_quals.is_empty() {
            let build_conj = app_quals.len() > 1 ;
            let mut conj = Vec::with_capacity( app_quals.len() ) ;
            for term in app_quals.drain() {
              if let Some(max_var) = term.highest_var() {
                if build_conj { conj.push(term.clone()) }
                let arity: Arity = (1 + * max_var).into() ;
                // println!("- {}", term) ;
                quals.insert(arity, term) ;
              }
            }
            if build_conj {
              let term = term::and(conj) ;
              if let Some(max_var) = term.highest_var() {
                quals.insert( (1 + * max_var).into(), term )
              }
            }
          }

          maps.push(map)
        }
      }
    }

    // Build the conjunction of atoms.
    let mut conjs = vec![
      HConSet::<Term>::with_capacity( clause.lhs_terms.len() + 1 ) ;
      maps.len()
    ] ;

    // Stores the subterms of `lhs_terms` that are disjunctions or
    // conjunctions.
    let mut subterms = Vec::with_capacity(7) ;

    // Now look for atoms and try to apply the mappings above.
    for term in clause.lhs_terms.iter() {

      let mut cnt = 0 ;
      for map in & maps {
        if let Some( (term, true) ) = term.subst_total(map) {
          if let Some(max_var) = term.highest_var() {
            let arity: Arity = (1 + * max_var).into() ;
            conjs[cnt].insert( term.clone() ) ;
            cnt += 1 ;
            let term = if let Some(term) = term.rm_neg() {
              term
            } else { term } ;
            // println!("- {}", term) ;
            quals.insert(arity, term)
          }
        }
        // Is it a disjunction? If yes, add disjuncts as qualifiers.
        debug_assert!( subterms.is_empty() ) ;
        subterms.push(term) ;
        while let Some(subterm) = subterms.pop() {
          match subterm.app_inspect() {
            Some( (Op::Or, terms) ) |
            Some( (Op::And, terms) ) => for term in terms {
              subterms.push(term) ;
              if let Some( (qual, true) ) = term.subst_total(map) {
                if let Some(max_var) = qual.highest_var() {
                  let arity: Arity = (1 + * max_var).into() ;
                  let qual = if let Some(qual) = qual.rm_neg() {
                    qual
                  } else {
                    qual
                  } ;
                  quals.insert(arity, qual)
                }
              }
            },
            _ => (),
          }
        }
      }

    }

    for conj in conjs {
      if conj.len() > 1 {
        let term = term::and( conj.into_iter().collect() ) ;
        // println!("- {}", term) ;
        if let Some(max_var) = term.highest_var() {
          let arity: Arity = (1 + * max_var).into() ;
          quals.insert( arity, term )
        }
      }
    }

  }

  /// Turns a teacher counterexample into learning data.
  pub fn cexs_to_data(
    & self, data: & mut ::common::data::Data, cexs: Cexs
  ) -> Res<bool> {
    let mut nu_stuff = false ;
    for (clause, cex) in cexs.into_iter() {
      log_debug!{ "    working on clause {}...", clause }
      let clause = & self[clause] ;
      log_debug!{ "    getting antecedents..." }
      let mut antecedents = Vec::with_capacity( clause.lhs_len() ) ;
      log_debug!{ "    translating tterms..." }


      log_debug!{ "    working on lhs..." }
      for (pred, argss) in & clause.lhs_preds {
        let pred = * pred ;
        log_debug!{
          "        pred: {} / {} ({})",
          pred, self.preds.len(), self.pred_terms.len()
        }
        if self.pred_terms[pred].is_none() {
          log_debug!{ "        -> is none, {} args", argss.len() }
          for args in argss {
            let mut values = VarMap::with_capacity( args.len() ) ;
            for arg in args {
              values.push(
                arg.eval(& cex).chain_err(
                  || "during argument evaluation to generate learning data"
                ) ?
              )
            }
            antecedents.push(
              (pred, values)
            )
          }
        } else {
          log_debug!{ "      -> is some" }
        }
      }
      antecedents.shrink_to_fit() ;

      log_debug!{ "    working on rhs..." }
      let consequent = if let Some((pred, args)) = clause.rhs() {
        log_debug!{
          "        pred: {} / {} ({})",
          pred, self.preds.len(), self.pred_terms.len()
        }
        let mut values = VarMap::with_capacity( args.len() ) ;
        'pred_args: for arg in args {
          values.push(
            arg.eval(& cex).chain_err(
              || "during argument evaluation to generate learning data"
            ) ?
          )
        }
        Some( (pred, values) )
      } else {
        None
      } ;

      log_debug!{ "    antecedent: {:?}", antecedents }
      log_debug!{ "    consequent: {:?}", consequent }

      match ( antecedents.len(), consequent ) {
        (0, None) => bail!(
          ErrorKind::Unsat
        ),
        (1, None) => {
          let (pred, args) = antecedents.pop().unwrap() ;
          let new = data.stage_raw_neg(pred, args) ? ;
          nu_stuff = nu_stuff || new
        },
        (0, Some( (pred, args) )) => {
          let new = data.stage_raw_pos(pred, args) ? ;
          nu_stuff = nu_stuff || new
        },
        (_, consequent) => {
          let new = data.add_cstr(antecedents, consequent) ? ;
          nu_stuff = nu_stuff || new
        },
      }
    }

    Ok(nu_stuff)
  }



  /// Checks that the instance has no inconsistencies.
  ///
  /// Only active in debug.
  #[cfg(not(debug_assertions))]
  #[inline(always)]
  pub fn check(& self, _: & 'static str) -> Res<()> { Ok(()) }

  #[cfg(debug_assertions)]
  pub fn check(& self, s: & 'static str) -> Res<()> {
    for clause in & self.clauses {
      clause.check(s) ?
    }
    self.check_pred_to_clauses().chain_err(
      || format!("while checking `{}`", conf.sad("pred_to_clauses"))
    ).chain_err(
      || format!("instance consistency check failed: {}", conf.emph(s))
    ) ? ;
    self.check_preds_consistency() ? ;
    
    for clause in & self.clauses {
      for pred in clause.lhs_preds().iter().map(
        |(pred, _)| * pred
      ).chain( clause.rhs().into_iter().map(|(pred, _)| pred) ) {
        if let Some(tterms) = self.forced_terms_of(pred) {
          bail! {
            "predicate {} is forced{} but appears in a clause: {}",
            conf.bad( & self[pred].name ),
            match tterms.bool() {
              Some(true) => " to true",
              Some(false) => " to false",
              None => "",
            },
            s
          }
        }
      }
    }

    Ok(())
  }

  /// Checks `preds` and `old_preds` are consistent.
  #[cfg(debug_assertions)]
  fn check_preds_consistency(& self) -> Res<()> {
    for (pred, info) in self.preds.index_iter() {
      for (var, typ) in info.sig.index_iter() {
        let (ref old_sig, ref var_map) = self.old_preds[pred] ;
        if old_sig[ var_map[var] ] != * typ {
          bail!(
            "type inconsistency between current and original predicates:\n\
            on {}, variable {}: {} is routed to {}: {}",
            self[pred],
            var.default_str(), typ,
            var_map[var].default_str(), old_sig[ var_map[var] ]
          )
        }
      }
    }
    Ok(())
  }

  /// Pretty printer for a set of clauses.
  #[cfg(debug_assertions)]
  fn pretty_clauses(& self, clauses: & ClsSet) -> String {
    let mut s = String::new() ;
    s.push('{') ;
    for clause in clauses {
      s.push(' ') ;
      s.push_str(& format!("{}", clause))
    }
    s.push(' ') ;
    s.push('}') ;
    s
  }

  /// Checks the consistency of `pred_to_clauses`.
  #[cfg(debug_assertions)]
  fn check_pred_to_clauses(& self) -> Res<()> {
    for (cls_idx, clause) in self.clauses.index_iter() {
      for (pred, _) in & clause.lhs_preds {
        let pred = * pred ;
        if self.is_known(pred) {
          bail!(
            "predicate {} is forced but appears in lhs of clause {}",
            self[pred], cls_idx
          )
        }
        if ! self.pred_to_clauses[pred].0.contains(& cls_idx) {
          bail!(
            "predicate {} appears in lhs of clause {} \
            but is not registered as such\n{}\nlhs: {}\nrhs: {}",
            self[pred], cls_idx,
            self.clauses[cls_idx].to_string_info(self.preds()) ?,
            self.pretty_clauses(
              & self.pred_to_clauses[pred].0
            ), self.pretty_clauses(
              & self.pred_to_clauses[pred].1
            )
          )
        }
      }
      if let Some((pred, _)) = clause.rhs() {
        if self.is_known(pred) {
          bail!(
            "predicate {} is forced but appears in rhs of clause {}",
            self[pred], cls_idx
          )
        }
        if ! self.pred_to_clauses[pred].1.contains(& cls_idx) {
          bail!(
            "predicate {} appears in rhs of clause {} \
            but is not registered as such\n{}\nlhs: {}\nrhs: {}",
            self[pred], cls_idx,
            self.clauses[cls_idx].to_string_info(self.preds()) ?,
            self.pretty_clauses(
              & self.pred_to_clauses[pred].0
            ), self.pretty_clauses(
              & self.pred_to_clauses[pred].1
            )
          )
        }
      }
    }

    for (pred, & (ref lhs, ref rhs)) in self.pred_to_clauses.index_iter() {
      'pred_clauses: for clause in lhs {
        if * clause >= self.clauses.len() {
          bail!(
            "predicate {} is registered as appearing in lhs of clause {} \
            which is above the maximal clause index", self[pred], clause
          )
        }
        if self.clauses[* clause].lhs_preds.get(& pred).is_none() {
          bail!(
            "predicate {} is registered as appearing in lhs of clause {} \
            but it's not the case\n{}\nlhs: {}\nrhs: {}",
            self[pred], clause,
            self.clauses[* clause].to_string_info(self.preds()) ?,
            self.pretty_clauses(
              & self.pred_to_clauses[pred].0
            ), self.pretty_clauses(
              & self.pred_to_clauses[pred].1
            )
          )
        }
      }
      for clause in rhs {
        if * clause >= self.clauses.len() {
          bail!(
            "predicate {} is registered as appearing in rhs of clause {} \
            which is above the maximal clause index", self[pred], clause
          )
        }
        if let Some((this_pred, _)) = self.clauses[* clause].rhs() {
          if this_pred == pred {
            continue
          }
        }
        bail!(
          "predicate {} is registered to appear in rhs of clause {} \
          but it's not the case\n{}\nlhs: {}\nrhs: {}",
          self[pred], clause,
          self.clauses[* clause].to_string_info(self.preds()) ?,
          self.pretty_clauses(
            & self.pred_to_clauses[pred].0
          ), self.pretty_clauses(
            & self.pred_to_clauses[pred].1
          )
        )
      }
    }
    Ok(())
  }


  /// Dumps the instance as an SMT-LIB 2 problem.
  pub fn dump_as_smt2<File, Blah>(
    & self, w: & mut File, blah: Blah
  ) -> Res<()>
  where File: Write, Blah: AsRef<str> {
    use common::consts::keywords ;
    let blah = blah.as_ref() ;

    for line in blah.lines() {
      write!(w, "; {}\n", line) ?
    }
    write!(w, "\n") ? ;

    for (pred_idx, pred) in self.preds.index_iter() {
      if self.pred_terms[pred_idx].is_none() {
        write!(
          w, "({}\n  {}\n  (", keywords::prd_dec, pred.name
        ) ? ;
        for typ in & pred.sig {
          write!(w, " {}", typ) ?
        }
        write!(w, " ) Bool\n)\n") ?
      }
    }

    for (idx, clause) in self.clauses.index_iter() {
      write!(w, "\n; Clause #{}\n", idx) ? ;
      clause.write(
        w, |w, p, args| {
          write!(w, "(") ? ;
          w.write_all( self[p].name.as_bytes() ) ? ;
          for arg in args {
            write!(w, " ") ? ;
            arg.write(w, |w, var| w.write_all( clause.vars[var].as_bytes() )) ?
          }
          write!(w, ")")
        }
      ) ? ;
      write!(w, "\n\n") ?
    }

    write!(w, "\n(check-sat)\n") ? ;

    Ok(())
  }

  /// Writes a model.
  pub fn write_model<W: Write>(& self, model: & Model, w: & mut W) -> Res<()> {
    writeln!(w, "(model") ? ;
    for & (pred, ref tterms) in model {
      let pred_info = & self[pred] ;
      let (ref old_sig, ref var_map) = self.old_preds[pred] ;
      // Reverse `var_map` so that it maps old vars to new ones.
      let mut pam_rav = VarHMap::with_capacity( var_map.len() ) ;
      for (new, old) in var_map.index_iter() {
        let prev = pam_rav.insert( * old, new ) ;
        debug_assert!( prev.is_none() )
      }

      writeln!(
        w, "  ({} {}", ::common::consts::keywords::prd_def, pred_info.name
      ) ? ;
      write!(w, "    (")  ?;

      for (var, typ) in old_sig.index_iter() {
        write!(w, " (") ? ;
        if let Some(var) = pam_rav.remove(& var) {
          write!(w, "{}", term::var(var)) ?
        } else {
          write!(w, "unused_{}", var) ?
        }
        write!(w, " {})", typ) ?
      }

      writeln!(w, " ) {}", Typ::Bool) ? ;
      write!(w, "    ") ? ;
      self.print_tterms_as_model(w, tterms) ? ;
      writeln!(w, "\n  )") ?
    }
    writeln!(w, ")") ? ;
    Ok(())
  }
}
impl ::std::ops::Index<PrdIdx> for Instance {
  type Output = PrdInfo ;
  fn index(& self, index: PrdIdx) -> & PrdInfo {
    & self.preds[index]
  }
}
impl ::std::ops::Index<ClsIdx> for Instance {
  type Output = Clause ;
  fn index(& self, index: ClsIdx) -> & Clause {
    & self.clauses[index]
  }
}
impl ::std::ops::IndexMut<ClsIdx> for Instance {
  fn index_mut(& mut self, index: ClsIdx) -> & mut Clause {
    & mut self.clauses[index]
  }
}








// impl<'a> PebcakFmt<'a> for TTerm {
//   type Info = (& 'a VarMap<VarInfo>, & 'a PrdMap<PrdInfo>) ;
//   fn pebcak_err(& self) -> ErrorKind {
//     "during top term pebcak formatting".into()
//   }
//   fn pebcak_io_fmt<W: Write>(
//     & self, w: & mut W,
//     (vars, prds): (& 'a VarMap<VarInfo>, & 'a PrdMap<PrdInfo>)
//   ) -> IoRes<()> {
//     self.write(
//       w,
//       |w, var| w.write_all( vars[var].as_bytes() ),
//       |w, prd| w.write_all( prds[prd].as_bytes() )
//     )
//   }
// }

impl<'a> PebcakFmt<'a> for Clause {
  type Info = & 'a PrdMap<PrdInfo> ;
  fn pebcak_err(& self) -> ErrorKind {
    "during clause pebcak formatting".into()
  }
  fn pebcak_io_fmt<W: Write>(
    & self, w: & mut W, prds: & 'a PrdMap<PrdInfo>
  ) -> IoRes<()> {
    self.write(
      w, |w, prd, args| {
        write!(w, "(") ? ;
        w.write_all( prds[prd].as_bytes() ) ? ;
        for arg in args {
          write!(w, " ") ? ;
          arg.write(w, |w, var| w.write_all( self.vars[var].as_bytes() )) ?
        }
        write!(w, ")")
      }
    )
  }
}

impl<'a> PebcakFmt<'a> for Instance {
  type Info = () ;
  fn pebcak_err(& self) -> ErrorKind {
    "during instance pebcak formatting".into()
  }
  fn pebcak_io_fmt<W: Write>(
    & self, w: & mut W, _: ()
  ) -> IoRes<()> {
    use common::consts::keywords ;

    for (pred_idx, pred) in self.preds.index_iter() {
      if self.pred_terms[pred_idx].is_none() {
        write!(
          w, "({}\n  {}\n  (", keywords::prd_dec, pred.name
        ) ? ;
        for typ in & pred.sig {
          write!(w, " {}", typ) ?
        }
        write!(w, " ) Bool\n)\n") ? ;
        if pred.sig.len() != self.old_preds[pred_idx].0.len() {
          write!(w, "; original signature:\n;") ? ;
          for (var, typ) in self.old_preds[pred_idx].0.index_iter() {
            write!(
              w, " ({} {})", var.default_str(), typ
            ) ?
          }
          writeln!(w, "\n; variable map (new -> old):\n;") ? ;
          for (src, tgt) in self.old_preds[pred_idx].1.index_iter() {
            write!(
              w, " {} -> {},", src.default_str(), tgt.default_str()
            ) ?
          }
          writeln!(w, "") ?
        }
      }
    }

    use rsmt2::to_smt::Expr2Smt ;
    let empty_prd_set = PrdSet::new() ;
    if self.sorted_pred_terms.is_empty() {
      // Either there's no forced predicate, or we are printing before
      // finalizing.
      for (pred, tterms) in self.pred_terms.index_iter().filter_map(
        |(pred, tterms_opt)| tterms_opt.as_ref().map(|tt| (pred, tt))
      ) {
        write!(w, "({} {}\n  (", keywords::prd_def, self[pred]) ? ;
        for (var, typ) in self[pred].sig.index_iter() {
          write!(w, " (v_{} {})", var, typ) ?
        }
        write!(w, " ) Bool\n  ") ? ;
        tterms.expr_to_smt2(
          w, & (& empty_prd_set, & empty_prd_set, & self.preds)
        ).unwrap() ;
        write!(w, "\n)\n") ?
      }
    } else {
      for pred in & self.sorted_pred_terms {
        write!(w, "({} {}\n  (", keywords::prd_def, self[* pred]) ? ;
        for (var, typ) in self[* pred].sig.index_iter() {
          write!(w, " (v_{} {})", var, typ) ?
        }
        let tterms = self.pred_terms[* pred].as_ref().unwrap() ;
        write!(w, " ) Bool\n  ") ? ;
        tterms.expr_to_smt2(
          w, & (& empty_prd_set, & empty_prd_set, & self.preds)
        ).unwrap() ;
        write!(w, "\n)\n", ) ?
      }
    }

    for (idx, clause) in self.clauses.index_iter() {
      write!(w, "\n; Clause #{}\n", idx) ? ;
      clause.pebcak_io_fmt(w, & self.preds) ?
    }

    write!(w, "\npred to clauses:\n") ? ;
    for (pred, & (ref lhs, ref rhs)) in self.pred_to_clauses.index_iter() {
      write!(w, "  {}: lhs {{", self[pred]) ? ;
      for lhs in lhs {
        write!(w, " {}", lhs) ?
      }
      write!(w, " }}, rhs {{") ? ;
      for rhs in rhs {
        write!(w, " {}", rhs) ?
      }
      write!(w, " }}\n") ?
    }

    Ok(())
  }
}
