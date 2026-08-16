# ra2-trainer

Trainer para Command & Conquer Red Alert 2 / Yuri's Revenge corriendo en Linux
(vía Wine/Proton, por ejemplo la versión de Steam). Tiene una UI de escritorio
(Angular + Tauri/Rust) que parchea la memoria del proceso del juego en
caliente para activar los cheats, sin depender de `gdb` ni de scripts de
shell sueltos.

## Cheats disponibles

| Cheat | Descripción | Versiones soportadas |
|---|---|---|
| Construir en cualquier lugar | Permite colocar estructuras fuera del rango de la base | RA2 v1.000/v1.006 y Yuri's Revenge v1.000 |
| Créditos infinitos | Detiene la disminución de créditos | RA2 v1.000/v1.006 y Yuri's Revenge v1.000 |
| Revelar mapa completo | Revela el mapa completo mientras poseas alguna estructura | RA2 v1.000/v1.006 (no Yuri's Revenge) |
| Radar siempre activo | Activa el mapa de radar incondicionalmente | RA2 v1.000/v1.006 (no Yuri's Revenge) |
| Energía infinita | Congela el consumo de energía en 0 (nunca falta energía, sin importar cuántos edificios tengas) | RA2 v1.006 y Yuri's Revenge (build 1.11 de Steam) |
| Construcción instantánea | Completa al instante cualquier edificio o unidad en cola (propia y de la IA), mientras esté activado | RA2 v1.006 |

Los primeros cinco son parches estáticos de bytes (se aplican una vez y
quedan aplicados hasta que cierres el juego). "Construcción instantánea" es
distinto: usa un breakpoint en vivo y hay que dejarlo activado (toggle) para
que siga teniendo efecto.

> **Nota de versiones**: las direcciones de "Energía infinita" se
> verificaron por desensamblado directo contra los binarios de la instalación
> de Steam actual (`game.exe` reporta FileVersion 1.08 mapeando al mismo
> código que ya usaba `infinite-credits` como "v1.006"; `gamemd.exe` reporta
> FileVersion 1.11). Al hacerlo se detectó que las direcciones existentes de
> `build-anywhere`/`infinite-credits` para "Yuri's Revenge v1.000" **no
> coinciden** con el `gamemd.exe` de esta instalación (el sitio real de
> `SpendMoney` está en otra dirección) — probablemente esos dos cheats
> aparezcan como "unsupported" en Yuri's Revenge hasta que se rederiven sus
> direcciones contra el build actual.

## Cómo funciona

- **UI**: Angular, en `ui/src/app/`.
- **Backend**: Rust (Tauri), en `ui/src-tauri/src/`. Detecta el proceso
  `game.exe`/`gamemd.exe`, lee/escribe su memoria vía `ptrace` +
  `/proc/<pid>/mem`, y expone todo a la UI como comandos de Tauri.
- **Helper privilegiado**: como los sistemas Linux modernos restringen
  `ptrace` a procesos hijos (`ptrace_scope`), las operaciones de memoria
  corren en un binario aparte (`ra2-trainer-helper`) elevado vía `pkexec`.
  Este helper se lanza **una sola vez por sesión** (un solo pedido de
  autenticación) y se mantiene corriendo, comunicándose con la app por
  stdin/stdout, para no pedir contraseña en cada click.
- **Construcción instantánea**: en vez de parchear bytes de código, instala
  un breakpoint real (`int3`) en la instrucción que actualiza el progreso de
  construcción y fuerza el valor a "completo" cada vez que se ejecuta,
  restaurando el byte original al desactivarlo o cerrar la app.

## Requisitos

- Linux con `pkexec`/PolicyKit instalado.
- Node.js + npm.
- Rust + Cargo (`rustc`, `cargo`).
- El juego corriendo bajo Wine/Proton (probado con la versión de Steam).

## Cómo levantar la UI (desarrollo)

```sh
cd ui
npm install
npm run dev
```

Esto compila y levanta la app de escritorio completa (Angular + backend Rust)
en modo desarrollo, con recarga automática. La primera vez que uses una
acción que toca memoria del juego (refrescar estado, aplicar un cheat,
activar construcción instantánea), `pkexec` va a pedir autenticación una
vez; las siguientes acciones reusan esa misma sesión elevada sin volver a
preguntar.

## Build de producción

```sh
cd ui
npm run tauri build
```

Genera el ejecutable en `ui/src-tauri/target/release/app` (y de paso
`.deb`/`.rpm`/`.AppImage` en `target/release/bundle/`).

> **Importante**: usá siempre `npm run tauri build` (o `cargo build
> --release --bin app` pero corrido *desde* `npm run tauri build` al menos
> una vez antes). Un `cargo build --release` suelto, sin pasar por la CLI de
> Tauri, no ejecuta el `beforeBuildCommand` (compilar Angular) ni fija las
> variables de entorno que le dicen al binario que sirva los archivos ya
> compilados en vez de buscar el dev server en `localhost:4200` — el
> síntoma es pantalla en blanco con "Could not connect to localhost:
> Connection refused" al abrir la app.

También hace falta el helper compilado en release:

```sh
cd ui/src-tauri
cargo build --release --bin ra2-trainer-helper
```

## Ícono y acceso directo

El ícono de la app está en `ui/src-tauri/icons/` (generado a partir de
`ui/src-tauri/icon-source.svg` con `npx tauri icon`). Hay un lanzador de
escritorio listo en `ra2-trainer.desktop`, apuntando al binario release y a
ese ícono. Para instalarlo:

```sh
cp ra2-trainer.desktop ~/.local/share/applications/
```

(Requiere haber compilado antes el binario release, ver arriba.) Después de
copiarlo debería aparecer "RA2 Trainer" con su ícono en el menú de
aplicaciones.

## Uso

1. Iniciá el juego (Red Alert 2 o Yuri's Revenge) y dejalo en una partida.
2. Abrí la app: debería detectar automáticamente el proceso (`game.exe` o
   `gamemd.exe`). Si la detección automática falla, hay un campo para
   ingresar el PID manualmente.
3. Tocá "Refrescar estado" para ver qué versión detectó y el estado de cada
   cheat.
4. Aplicá los cheats que quieras, o activá el toggle de construcción
   instantánea.
