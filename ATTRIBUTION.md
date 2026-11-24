# ATTRIBUTION.md — Registro de Assets e Propriedade Intelectual

Este arquivo cataloga todos os assets embutidos (texturas, mapas ópticos, LUTs, fontes, ícones e presets) e confirma a conformidade de licenças SPDX conforme [`AGENTS.md`](AGENTS.md) (§1.2).

## Assets Embutidos

Atualmente o repositório contém exclusivamente código-fonte original e algoritmos procedurais em Rust nativo; **nenhum asset binário externo** foi adicionado até o momento.

Sempre que um asset `X` for incluído:
1. Crie `X.attribution` ao lado do arquivo seguindo o formato definido em `AGENTS.md` §1.2.
2. Registre a entrada na tabela abaixo com identificador SPDX da lista permitida.

| Asset | Título | Autor / Detentor | Origem | Licença SPDX | Adicionado em |
| --- | --- | --- | --- | --- | --- |
| *(nenhum asset externo adicionado)* | — | — | — | — | — |

## Especificações Públicas de Referência

- **OpenFX Image Effect Plugin API 1.4** — Especificação aberta mantida pela Academy Software Foundation (BSD-3-Clause). As definições ABI em `crates/bulldog_ofx/src/abi.rs` são implementadas em sala limpa em Rust a partir da especificação pública do padrão OpenFX.

## Dependências de Código (Cargo)

Todas as dependências diretas e transitivas resolvidas em `Cargo.lock` são auditadas automaticamente pelo comando `cargo run -p bulldog_packager -- check-licenses` contra a lista SPDX permitida em `AGENTS.md`.
