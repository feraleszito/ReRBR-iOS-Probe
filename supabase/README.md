# Desplegar la clasificación online

El cliente experimental usa la URL confirmada en **Connect** y la clave
publicable de `mobile/leaderboard/config.js`. La clave administrativa que usa
la función proviene exclusivamente de su entorno de ejecución en Supabase.

1. En el proyecto **RBR-Mobile-Leaderboards**, abre **SQL Editor** y ejecuta
   `supabase/inspect-leaderboard.sql`. Confirma los tipos, políticas y permisos
   de `players` y `stage_times` con el esquema inicial. Conserva la copia de
   seguridad habitual del proyecto.
2. En SQL Editor ejecuta una vez, en orden,
   `migrations/202611250001_online_leaderboards.sql` y
   `migrations/202611250002_catalog.sql`. El primero añade un identificador
   de envío nullable e instala las funciones. El segundo carga el catálogo
   de referencia y el número de parciales de cada tramo. Los tiempos anteriores
   conservan su contenido. Los archivos de `tests/` son exclusivamente para
   una base desechable.
3. Desde la raíz de la rama de integración, con la CLI de Supabase autenticada
   en una cuenta con acceso a este proyecto, ejecuta:

   ```sh
   supabase functions deploy submit-stage-time --project-ref qeptaxipcklgsqnrunak --no-verify-jwt
   ```

   La opción selecciona la comprobación del token dentro de la función mediante
   `auth.getUser(bearer)`, compatible con las claves de firma del proyecto.
   La identidad del usuario validado es la única que recibe la operación de
   escritura. Supabase proporciona `SUPABASE_URL`, `SUPABASE_ANON_KEY` y
   `SUPABASE_SERVICE_ROLE_KEY` a la función. En **Authentication → Sign In /
   Providers** debe estar habilitado **Anonymous Sign-Ins**.
4. En iPad abre **Clasificación de tiempos → Clasificación online**, elige nombre,
   completa un tramo admitido y abre de nuevo el mismo tramo. En Android,
   con otra sesión, selecciona ese tramo y **Todos los vehículos**. Comprueba
   nombre, tiempo, penalización y estado provisional. Repite tras terminar
   con modo avión y recuperar la conexión. Un reintento conserva el ID original.

La consulta devuelve el mejor resultado por jugador, páginas de 25 y el mejor
tiempo y posición de la sesión. Los empates comparten puesto. El filtro de coche
usa el ID del catálogo. El almacenamiento seguro guarda el refresh token y el
access token en Keychain o con cifrado AES-GCM y Android Keystore. Mantener la
misma identidad anónima permite añadir una identidad permanente mediante
`linkIdentity` más adelante.

La función acepta resultados provisionales. Comprobar parciales y catálogo o
deduplicar un envío no prueba que la carrera sea legítima. El campo
`verified` se inicializa en falso. Para añadir contenido, exporta la lista de
una instalación de referencia con `cargo run --example leaderboard-catalog
--features public-beta -- <carpeta-que-contiene-RBR>` y revisa las filas antes
de aplicarlas. El APK y la IPA no incluyen recursos comerciales.
