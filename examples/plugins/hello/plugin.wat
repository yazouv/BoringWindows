;; Plugin d'exemple : affiche un texte et demande un peu d'attention.
;; Assembler : voir docs/fr/src/plugins.md (wat2wasm, ou n'importe quel langage qui vise WASM).
(module
  (import "bw" "set_text" (func $set_text (param i32 i32)))
  (import "bw" "set_attention" (func $set_attention (param i32)))
  (import "bw" "log" (func $log (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 16) "Bonjour depuis un plugin WASM")
  (data (i32.const 64) "bw_update appele")
  (func (export "bw_update")
    (call $log (i32.const 64) (i32.const 16))
    (call $set_text (i32.const 16) (i32.const 29))
    (call $set_attention (i32.const 1))))
