# Workspace CLI: Orquestador Nativo de Procesos para Windows

Un gestor de entornos de trabajo concurrente escrito en **Rust**. Esta herramienta interactúa directamente con la **Win32 API** para lanzar, rastrear y posicionar aplicaciones en configuraciones multi-monitor de forma automatizada mediante perfiles JSON.

## Arquitectura y Retos Técnicos Resueltos

* **Concurrencia Segura y Aislamiento de Pánicos:**
  El motor asíncrono utiliza `std::thread` para lanzar múltiples aplicaciones en paralelo. Cada hilo captura su propio `Result<AppResult, anyhow::Error>`, garantizando que el fallo o *timeout* de una aplicación individual (ej. rutas inválidas) no bloquee ni tumbe el despliegue del resto del entorno.

* **Manipulación Segura de la Memoria (Win32 API):**
  Para cumplir con las garantías de concurrencia de Rust (`Send` y `Sync`), los punteros de ventana (`HWND`) no se exponen como handles crudos entre hilos. Se resuelven y empaquetan en tiempo de ejecución como tipos primitivos (`isize`), confinando el código `unsafe` exclusivamente a los callbacks del sistema.

* **Evasión de Procesos Singleton (Electron/Modern Apps):**
  Aplicaciones como VS Code, Claude o el nuevo Bloc de notas de Windows 11 utilizan un modelo de instancia única. Si ya hay un proceso en segundo plano, el orquestador detecta la muerte prematura del lanzador y hace un *fallback* automático a `Toolhelp32Snapshot` para rastrear el árbol de procesos huérfanos y cazar la nueva ventana real.

* **Topología Multi-Monitor (GDI):**
  Lectura dinámica de la topología de pantallas usando `EnumDisplayMonitors` sin fugas de memoria (descartando `HDC` y `HMONITOR` tras el callback). El sistema traduce coordenadas relativas de configuración (ej. "Monitor 2, x: 0") a coordenadas absolutas del escritorio virtual respetando la barra de tareas (`rcWork`).

## Guía Rápida de Uso

### 1. Compilación y Preparación
Clona el repositorio y compila la versión optimizada para Windows:

```powershell
cargo build --release
```

El ejecutable final se generará en `target/release/workspace-cli.exe`. Muévelo junto a tu archivo de configuración a una carpeta permanente (ej. `C:\Tools\WorkspaceManager`).

### 2. Configuración del Perfil
Renombra o copia el archivo `workspace.example.json` a `workspace.json`. Ajusta las rutas absolutas (`path`) para que apunten a los ejecutables de tus aplicaciones locales.

### 3. Ejecución y Automatización
Puedes lanzar el orquestador directamente desde una terminal:

```powershell
workspace-cli start dev_flow --config ./workspace.json
```

**Integración nativa recomendada:** 
Crea un acceso directo en tu escritorio apuntando al ejecutable e inyecta los argumentos en el campo *Destino*:
`"C:\ruta\workspace-cli.exe" start dev_flow --config "C:\ruta\workspace.json"`
* Configura las propiedades del acceso directo para que se ejecute **Minimizada**.
* Asigna una **Tecla de método abreviado** (ej. `Ctrl + Alt + W`) para desplegar tu entorno al instante y en segundo plano.