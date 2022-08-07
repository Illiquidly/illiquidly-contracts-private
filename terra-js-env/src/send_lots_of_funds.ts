import { Address } from './terra_utils';
import { env } from './env_helper';
import { MsgSend } from "@terra-money/terra.js"

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let all_handlers: Address[] = env['mnemonics'].map((mnemonic: string) => {
    return new Address(mnemonic);
  });

  let all_msg = all_handlers.flatMap((h)=>{
    let msgs = [];
    for(let i=0;i<54;i++){
      msgs.push(new MsgSend(handler.getAddress(), h.getAddress(), {
        uluna: '5000'
      }))
    }
    return msgs;
  })

  let response = await handler.post(all_msg);
  console.log(response)

}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
