# CLAUDE.md — Guia de Trabalho do Bulldog Complements

Este arquivo complementa [`AGENTS.md`](AGENTS.md) com comandos e padrões práticos do dia a dia. **Quando houver qualquer divergência, `AGENTS.md` prevalece sempre.**

## Estrutura do Workspace

- `crates/bulldog_core` — Núcleo puro Rust (`#![forbid(unsafe_code)]`): buffers seguros com suporte a *stride* positivo/negativo (`row_bytes`), pipeline linear `f32`, conversões de cor (`Linear`, `sRGB`, `Rec.709`, `Rec.2020`), alfa pré-multiplicado vs reto, PRNG/hash determinístico, pool de buffers reutilizáveis, paralelismo via `rayon` com `OfxAbort` e primitivos DSP.
- `crates/bulldog_ofx` — ABI C OpenFX 1.4 limpa (especificação aberta ASWF), dispatcher completo de ciclo de vida com `catch_unwind` e recuperação de `PoisonError`, macro `export_ofx_suite!`, validador de metadados/clean-room e *mock host* OFX em memória para testes de ponta a ponta.
- `crates/bulldog_packager` — Ferramenta de linha de comando para validar catálogo, auditar licenças SPDX e empacotar/verificar os bundles `.ofx.bundle` (incluindo checagem de símbolos exportados `OfxGetNumberOfPlugins` e `OfxGetPlugin`).
- `crates/<suite>` — 9 suítes independentes (`rlib` puros, sem colisões de símbolos C ABI):
  `continuus`, `sapphirus`, `dulcia_nitentia`, `caeruleus_nebulosus`, `transitions_3d`, `tracker_man`, `glass_brokers`, `universum`, `particulae_legales`.
- `bundles/<suite>_ofx` — Camadas finas `cdylib` que exportam exclusivamente `OfxGetNumberOfPlugins` e `OfxGetPlugin` para cada `.ofx.bundle`.

## Comandos Essenciais (Portões de Qualidade)

Execute todos os portões antes de abrir ou atualizar um Pull Request:

```bash
# 1. Formatação
cargo fmt --all -- --check

# 2. Lints estritos (zero panic/unwrap/expect/indexing em produção)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Testes unitários, hostis e ciclo de vida OpenFX completo
cargo test --workspace

# 4. Validação de catálogo e licenças SPDX
cargo run -p bulldog_packager -- validate-catalog
cargo run -p bulldog_packager -- check-licenses

# 5. Build e empacotamento dos bundles .ofx.bundle + verificação de símbolos exportados
cargo build --workspace --release
cargo run -p bulldog_packager -- package --profile release --out-dir dist
```

## Como Adicionar um Novo Efeito ou Transição

1. Implemente a struct do efeito implementando `bulldog_core::EffectPlugin` dentro da suíte apropriada em `crates/<suite>/src/`.
2. Utilize prefixo único `br.com.bulldog.complements.<suite>.<efeito>` em `snake_case`, nomes e descrições 100% originais (zero marcas de terceiros).
3. Garanta independência de resolução (parâmetros normalizados), processamento linear `f32`, preservação de alfa, determinismo temporal (`frame_time` + `seed`), `is_identity`, `region_of_definition`/`regions_of_interest` (quando expandir bordas) e checagem periódica de `ctx.abort`.
4. Para transições (`EffectContext::Transition`), declare obrigatoriamente o parâmetro `"Transition"` (`0.0..=1.0`) e retorne `IsIdentity` em `0.0` (`SourceFrom`) e `1.0` (`SourceTo`).
5. Registre o efeito em `suite_plugins()` e na macro `bulldog_ofx::register_ofx_suite!(...)` da suíte (o `cdylib` em `bundles/<suite>_ofx` o exportará automaticamente).
6. Adicione testes cobrindo funcionalidade, entradas hostis (`0x0`, `1x1`, `NaN`, strides negativos), determinismo e ciclo de vida OFX.
7. Atualize `CATALOG.md` e `README.md`.
