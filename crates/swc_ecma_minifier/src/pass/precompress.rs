use swc_atoms::js_word;
use swc_common::util::take::Take;
use swc_ecma_ast::*;
use swc_ecma_transforms_base::{
    helpers::{Helpers, HELPERS},
    perf::Parallel,
};
use swc_ecma_visit::{noop_visit_mut_type, VisitMut, VisitMutWith, VisitWith};

use crate::{
    analyzer::{ModuleInfo, UsageAnalyzer},
    marks::Marks,
    option::CompressOptions,
    util::ModuleItemExt,
};

/// Optimizer invoked before invoking compressor.
///
/// - Remove parens.
pub(crate) fn precompress_optimizer<'a>(
    module_info: &'a ModuleInfo,
    options: &'a CompressOptions,
    marks: Marks,
) -> impl 'a + VisitMut {
    PrecompressOptimizer {
        options,
        module_info,
        marks,
        ctx: Default::default(),
    }
}

#[derive(Debug)]
pub(crate) struct PrecompressOptimizer<'a> {
    options: &'a CompressOptions,
    module_info: &'a ModuleInfo,

    marks: Marks,

    ctx: Ctx,
}

impl Parallel for PrecompressOptimizer<'_> {
    fn create(&self) -> Self {
        Self { ..*self }
    }

    fn merge(&mut self, _: Self) {}
}

#[derive(Debug, Default, Clone, Copy)]
struct Ctx {
    in_var_pat: bool,
}

impl PrecompressOptimizer<'_> {
    fn handle_stmts<T>(&mut self, stmts: &mut Vec<T>)
    where
        T: for<'aa> VisitMutWith<PrecompressOptimizer<'aa>> + ModuleItemExt,
        Vec<T>: for<'aa> VisitMutWith<PrecompressOptimizer<'aa>> + VisitWith<UsageAnalyzer>,
    {
        stmts.visit_mut_children_with(&mut PrecompressOptimizer {
            options: self.options,
            module_info: self.module_info,
            marks: self.marks,
            ctx: self.ctx,
        });
    }
}

impl VisitMut for PrecompressOptimizer<'_> {
    noop_visit_mut_type!();

    fn visit_mut_decl(&mut self, n: &mut Decl) {
        n.visit_mut_children_with(self);

        if let Decl::Fn(FnDecl { ident, .. }) = n {
            if ident.sym == js_word!("") {
                n.take();
            }
        }
    }

    fn visit_mut_expr(&mut self, e: &mut Expr) {
        e.visit_mut_children_with(self);

        if let Expr::Paren(p) = e {
            *e = *p.expr.take();
        }
    }

    fn visit_mut_module_item(&mut self, n: &mut ModuleItem) {
        n.visit_mut_children_with(self);

        match n {
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
                decl: Decl::Var(var),
                ..
            })) if var.decls.is_empty() => {
                n.take();
            }
            _ => {}
        }
    }

    fn visit_mut_module_items(&mut self, n: &mut Vec<ModuleItem>) {
        HELPERS.set(&Helpers::new(true), || {
            self.handle_stmts(n);
        });

        n.retain(|s| !matches!(s, ModuleItem::Stmt(Stmt::Empty(..))));
    }

    fn visit_mut_stmt(&mut self, n: &mut Stmt) {
        n.visit_mut_children_with(self);

        match n {
            Stmt::Decl(Decl::Var(var)) if var.decls.is_empty() => {
                n.take();
            }
            _ => {}
        }
    }

    fn visit_mut_stmts(&mut self, n: &mut Vec<Stmt>) {
        self.handle_stmts(n);

        n.retain(|s| !matches!(s, Stmt::Empty(..)));
    }

    fn visit_mut_var_declarator(&mut self, n: &mut VarDeclarator) {
        let old = self.ctx;
        self.ctx.in_var_pat = true;
        n.name.visit_mut_with(self);

        self.ctx.in_var_pat = false;
        n.init.visit_mut_with(self);

        self.ctx = old;
    }
}
