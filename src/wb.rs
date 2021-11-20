// En este archivo crearemos el objeto de websocket 
use actix::{fut, ActorContext};

use crate::messages::{Disconnect, Connect, WsMessage, ClientActorMessage}; // Esto no va compilar, porque aun no hemos creado esto

use crate::lobby::Lobby; // Esto no va compilar, porque aun no hemos creado esto
use actix::{Actor, Addr, Running, StreamHandler, WrapFuture, ActorFuture, ContextFutureSpawner};
use actix::{AsyncContext, Handler};
use actix_web_actors::ws;
use actix_web_actors::ws::Message::Text;
use std::time::{Duration, Instant};
use uuid::Uuid;

// Definimos las constantes de unos procesos que tendrá la aplicación
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);


// Creamos la estructura para la websocket connection
struct WsConn {
    room: Uuid,
    lobby_addr: Addr<Lobby>,
    hb: Instant,
    id: Uuid,
}

/*
    room: Cada Socket existe en un ‘room’, el que en esta implementación solo será un simplen hashmap que mapea Uuid a la lista de Socket Ids.

    lobby_addr: Esta es la dirección del lobby en el que el socket existe. Será usado para enviar los datos al lobby, entonces enbiando un mensaje de texto al lobby luciria como `self.addr.do_send('hi!')`. Es importante para que el actor encuentre el lobby.

    hb: Aunque los WebSockets envían mensajes cuando se cierran, a veces los WebSockets se cierran sin previo aviso. en lugar de que ese actor exista para siempre, le enviamos un latido cada N segundos, y si no obtenemos una respuesta, terminamos el socket. Esta propiedad es el tiempo transcurrido desde que recibimos el último latido. En muchas bibliotecas, esto se maneja automáticamente.
    
    id: Es el ID asignado por nosotros para ese socket. Es util para, entre otras cosas, mensajese privados, podemos por ejemplo hacer /whisper <id> Hello!. Para susurrarse solo a ese cliente. 
*/

// Escribiremos un trait `new` para poder girar (spin up) los sockets un poco mas facilmente.

impl WsConn {
    pub fn new(room: Uuid, lobby: Addr<Lobby>) -> WsConn {
        WsConn {
            id: Uuid::new_v4(),
            room,
            hb: Instant::now(),
            lobby_addr: lobby,
        }
    }
}

// Entonces no tenemos que buscar como manejar ID o fijar un heartbit.



// Convirtiendolo en un actor
// Nos fijamos como WsConn es solo una estructura usual, para poderla convertir en un Actor, tenemos que implementar el `trait` Actor en el.


impl Actor for WsConn {

    // Eso es un mandato del actor. Ese es el contexto en el que vive este actor; aquí, estamos diciendo que el contexto es el contexto de WebSocket, y que se le debería permitir hacer cosas de WebSocket, como empezar a escuchar en un puerto. Definiremos un contexto antiguo simple en la sección de lobby en el futuro
    type Context = ws::WebsocketContext<Self>;

    // Escribimos el metodo para iniciar/crear el actor
    fn started(&mut self, ctx: &mut Self::Context) {

        // Iniciamos con un heartbeat loop, Funcion que se dispara en un intervalo.
        self.hb(ctx);

        // Tomamos la dirección de el lobby.
        let addr = ctx.address();
        
        self.lobby_addr

            // Es como mandar un mensaje diciendo:
            .send(Connect { // Si hacemos un do_send, seria convertirlo a un sync. Send necesita ser "esperado" await
                
                // Esta es la direccion de mi maibox puedes encontrarme
                addr: addr.recipient(),

                // El lobby al que quiero entrar
                lobby_id: self.room,

                // La identificacion donde me pueden encontrar
                self_id: self.id,
            })
            .into_actor(self)
            .then(|res, _, ctx| {
                match res {
                    Ok(_res) => (),

                    //  Si algo falla, simplemente detenemos todo el Actor con ctx.stop(), es probable que no suceda , pero algo malo podria pasar con el lobby
                    // Entonces si algo sale mal es mas facil detenerlo y enviamos un mensaje de dsesconeccion.
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
            // Nuestro WsConn ahora es actor.
    }

    // Escribimos el metodo para finalizar/destruir el actor
    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.lobby_addr.do_send(Disconnect { id: self.id, room_id: self.room });
        Running::Stop
    }
}