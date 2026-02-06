# Roadmap / TODO

Comparativa usada: VS Code, JetBrains (IntelliJ), Cursor y Zed.

## Prioridad P0 (bloqueantes para competir con IDEs maduros)

- [ ] Reescribir LSP para sesiones persistentes por workspace/lenguaje (no spawn por request).
- [ ] Hacer terminal/tareas/configuraciones no bloqueantes (ejecución async + streaming de salida), evitando `Command::output()` en hilo UI.
- [ ] Implementar depuración real con DAP (breakpoints, step in/out/over, variables y call stack real), reemplazando el call stack simulado.
- [ ] Añadir tests reales (unitarios + integración) para editor core, LSP parser, búsqueda/reemplazo y operaciones de archivo.
- [ ] Dividir `src/app.rs` en módulos por dominio (workspace, git, debug, settings, panels) para bajar riesgo de regresión.

## Prioridad P1 (paridad funcional con VS Code/JetBrains)

### Tabs & Title Bar
- [ ] Soporte de grupos de pestañas con drag&drop entre grupos persistente.
- [ ] Vista previa de archivo (single-click en explorer/quick open sin abrir tab permanente).
- [ ] Indicadores de conflictos de guardado/reload por pestaña.

### Editor UX
- [ ] Multi-cursor avanzado: box selection con mouse, skip current, add next/previous.
- [ ] Snippets completos con placeholders navegables (`Tab`/`Shift+Tab`) en lugar de strip de placeholders.
- [ ] Auto-indent por lenguaje (no solo heurística de `{([:`).
- [ ] Comentado/descomentado de línea y bloque.
- [ ] Bracket pair colorization y resaltado semántico de pares.
- [ ] Code folding por sintaxis/LSP (no solo heurística de líneas).

### Git Support
- [ ] Árbol de cambios agrupado por carpeta + vista side-by-side de diff.
- [ ] Soporte de conflictos de merge con acciones “Accept Current/Incoming/Both”.
- [ ] Rebase/cherry-pick interactivo y visual.
- [ ] Historial por archivo y navegación de blame por commit.

### Command Palette & Menus
- [ ] Unificar comandos de menú y palette (hoy hay muchas acciones fuera de la palette).
- [ ] Sistema de “when clauses”/context keys para habilitar comandos contextualizados.
- [ ] Quick Open global `Ctrl/Cmd+P` con fuzzy score robusto e historial reciente.

### Status Bar
- [ ] Estado LSP por lenguaje/proyecto con progreso real.
- [ ] Estado de tareas en background con cancelación.
- [ ] Estado de repo/branch por workspace root (multi-root real).

### Panels & Layout
- [ ] Explorer tipo árbol (actualmente listado plano limitado a 400 archivos).
- [ ] Layout persistente de paneles (anchos, tabs activas, split).
- [ ] Abrir resultados de búsqueda en panel dedicado reutilizable.

### Theme & Customization
- [ ] Sistema de temas token-based (scope colors), no solo paletas globales.
- [ ] Importación/exportación compatible con formatos de temas populares.
- [ ] Keybindings por plataforma con resolución de conflictos.

### Performance & Platform
- [ ] Indexación incremental del workspace (sin relanzar `rg --files` periódicamente).
- [ ] Watchers de FS nativos con debounce en vez de polling continuo.
- [ ] Evitar conversiones completas `rope -> String` para operaciones frecuentes.
- [ ] Perfilado de frame-time y budget por subsistema (render/input/LSP/git).

### Extensions & API
- [ ] API de extensiones versionada con permisos explícitos por capacidad.
- [ ] Aislamiento real de plugins (sandbox de procesos/FS/red), no solo variable `LUX_SANDBOX`.
- [ ] Marketplace con firma/verificación de manifiestos y checksum.

### File Management
- [ ] Guardado atómico (tmp + rename) y backup en fallos.
- [ ] “Safe delete” (papelera) + confirmación previa para borrado.
- [ ] Soporte robusto para archivos grandes y binarios.

### Search & Navigation
- [ ] Reemplazo por workspace con preview de diff y confirmación por archivo.
- [ ] Regex replace real en workspace.
- [ ] Corregir parsing de paths con `:` (Windows) en search, symbol y output links.
- [ ] Navegación a símbolo con index por lenguaje (LSP workspace symbols).

### Refactoring
- [ ] Refactors semánticos vía LSP (rename cross-file, extract function segura, inline variable segura).
- [ ] Preview de refactor multiarchivo con apply/reject por hunk.
- [ ] Historial de refactors con rollback transaccional.

### Debugging & Tasks
- [ ] Integrar `launch.json`/`tasks.json` compatibles (al menos subset VS Code).
- [ ] Variables/watch evaluadas por runtime real.
- [ ] Persistencia de breakpoints y run configs por workspace.

### Terminal & Output
- [ ] Terminal PTY real con procesos persistentes, señales y scrollback configurable.
- [ ] Soporte de múltiples terminales nombradas.
- [ ] Parser de enlaces de error por lenguaje/build tool.

### Diagnostics & Formatting
- [ ] Aplicar code actions y quick-fixes LSP reales (WorkspaceEdit).
- [ ] Soporte de `didChange` incremental y `publishDiagnostics` continuo.
- [ ] Configuración de formatter/linter por workspace, folder y language con precedencia clara.

### Collaboration
- [ ] Colaboración en tiempo real real (CRDT/OT + red), no archivo local `collab-*.json`.
- [ ] Presencia multiusuario, permisos de edición y comentarios sincronizados.
- [ ] Compartir sesión y follow mode.

### Accessibility
- [ ] Navegación completa por teclado en todos los paneles/modales.
- [ ] Compatibilidad con screen readers.
- [ ] Mejoras de contraste y escalado para low vision.

### Workspace & Projects
- [ ] Multi-root verdadero (comandos, búsqueda, tareas, git por root).
- [ ] Workspace files (`.lux-workspace`) con carpetas, settings y tasks.
- [ ] Trust model por folder con permisos granulares.

### Security & Privacy
- [ ] Migrar secretos a keychain del sistema (Keychain/CredMan/libsecret).
- [ ] Política de ejecución de scripts con prompts y allowlist por workspace.
- [ ] Sanitización fuerte de logs/export bundles para evitar exfiltración.

### Documentation & Help
- [ ] Documentar claramente qué está implementado vs experimental/simulado.
- [ ] Añadir troubleshooting por plataforma (macOS/Windows/Linux).
- [ ] Guías de migración desde VS Code/Sublime/JetBrains.

### Packaging & Distribution
- [ ] Sustituir placeholders de notarización/code signing por pipeline real.
- [ ] Instaladores nativos por plataforma (dmg/msi/deb/rpm) y auto-update real.
- [ ] Versionado semántico + release notes automáticas desde commits.

### Testing & QA
- [ ] Golden tests de rendering/editor interactions.
- [ ] Tests de regresión para undo/redo, multicursor y refactors.
- [ ] Tests end-to-end de flujos críticos (open/edit/save/search/git/lsp).
- [ ] Fuzzing para parser de LSP stream y operaciones de texto.

### CI/CD
- [ ] Cobertura de tests con umbral mínimo.
- [ ] Jobs de benchmark con alerta por regresión de rendimiento.
- [ ] Pipeline de release multiplataforma (no solo macOS).

### Internationalization
- [ ] Sistema i18n real para UI (catálogo de strings).
- [ ] Formatos locales de fecha/hora/mensajes.
- [ ] Verificación RTL completa en layout y editor widgets.

### Settings & Sync
- [ ] Merge semántico de settings (no solo last-write-wins por timestamp).
- [ ] Sync seguro cifrado opcional.
- [ ] Profiles exportables/importables por rol/proyecto.

### Onboarding & Templates
- [ ] Plantillas extensibles por lenguaje/framework.
- [ ] Onboarding contextual según stack detectado en workspace.
- [ ] Checklist “first run” persistente por versión.

### Observability
- [ ] Telemetría estructurada con eventos versionados y privacidad por defecto.
- [ ] Métricas locales de salud (LSP latency, frame drops, task duration).
- [ ] Crash reports con simbolización y deduplicación.

### Compatibility
- [ ] Compatibilidad más completa con `.editorconfig` (globs/secciones).
- [ ] Importador robusto de settings de VS Code/JetBrains/Sublime (schema-aware).
- [ ] Compatibilidad de keymaps y snippets populares.
