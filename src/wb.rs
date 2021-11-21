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

    // Ahora nuestro WsConn es un actor

    // Y ahora crearemos el latido heartbeat 


}


impl WsConn {

    // Todo lo que hacemos aquí es hacer ping al cliente y esperar una respuesta en un intervalo. Si la respuesta no llega, el enchufe murió; envíe una desconexión y detenga al cliente.
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("Disconnecting failed heartbeat");
                act.lobby_addr.do_send(Disconnect { id: act.id, room_id: act.room });
                ctx.stop();
                return;
            }

            ctx.ping(b"PING");
        });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConn {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        // Es una simple coincidencia de patrones en todos los posibles mensajes de WebSocket.
        match msg {
            // El ping responde con un pong, ese es el cliente que nos late. Responde con un pong. Como subproducto, dado que el cliente puede latirnos, sabemos que está vivo para que podamos restablecer nuestro reloj de latidos.
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            
            // El pong es la respuesta al ping que enviamos. Reinicia nuestro reloj, están vivos.
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }

            // Si el mensaje es binario, lo enviaremos al contexto de WebSocket que descubrirá qué hacer con él. Esto, de manera realista, nunca debería activarse.
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),

            // If it's a close message just close.
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }

            // Para este tutorial, no vamos a responder a los marcos de continuación (estos son, en resumen, mensajes de WebSocket que no caben en un solo mensaje)
            Ok(ws::Message::Continuation(_)) => {
                ctx.stop();
            }

            // On nop, nop (sin operación)
            Ok(ws::Message::Nop) => (),

            // En un mensaje de texto, (¡este es el que más haremos!) Envíelo al lobby. El lobby se ocupará de negociarlo hasta donde tenga que ir.
            Ok(Text(s)) => self.lobby_addr.do_send(ClientActorMessage {
                id: self.id,
                msg: s,
                room_id: self.room
            }),

            // En caso de error, entraremos en pánico. Probablemente desee implementar lo que debe hacer aquí de manera razonable.
            Err(e) => panic!(e),
        }
    }
}

// Si el servidor pone un `WsMessage` (El cual debemos definir), todo lo que haremos es enviar directamente al cliente.

// Así es como se ve "leer el correo" del buzón; impl Handler <MailType> para ACTOR
// Necesitamos definir cómo se verá la respuesta a ese correo. Si el correo se coloca como do_send, el tipo de respuesta no importa. Si se coloca como send(), entonces el tipo de resultado esperado será el resultado. tal vez escriba Result = String, o algo similar. Independientemente, sea cual sea el tipo T que coloque allí, el identificador debe devolver T.

impl Handler<WsMessage> for WsConn {
    // El The message en si mismo, tenemos el control total sobre que tanto o que tan poca informacion el mensaje permite mandar.
    type Result = ();

    // Self context. This is your own context, which is a "mailbox" of self. You can read memeber variables from the ctx, or you can put messages into your own mailbox here.
    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}