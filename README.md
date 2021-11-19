## Un simple chat room creado con actix websockets.


Se va a construir un simple chat room que responde mensajes a cada uno en la sala, con a habilidad de incluir mensajes privados.

En la arquitectura de atix tenemos dos componentes primarias, Actors y Messages. Podemos pensar a un actor como su propio objeto, con un buzon (mailbox). Actors pueden leer su mailbox y responder a su correo como corresponde, ya sea enviando un correo a otro actor, cambiando su estado o tal vez sin hacer nada en absoluto.
Actores: pequeñas cosas simples que leen y responden al correo.

Los actores trabajan completamente independientes unos de otros. Un actor puede estar en su propio hilo o en una máquina completamente diferente. Siempre que el actor pueda leer su correo, funciona perfectamente.

Es importante tener en cuenta que el actor solo existe en la memoria, con su dirección transmitida como `Addr<Actor>`. El propio Actor puede mutar sus propiedades (tal vez tenga una propiedad "messages_received" y necesite incrementarla en cada mensaje) pero no puede hacerlo en ningún otro lugar. En cambio, con el elemento `Addr<Actor>`, puede hacer `.send` (some_message) para poner un mensaje en el buzón de ese Actor.

En actix web, cada conexión de socket es su propio actor, y el "Lobby" es también su propio actor.

## Estructura

Cada enchufe se encuentra en una "room" y cada room se encuentra en una estructura de lobby singular.

