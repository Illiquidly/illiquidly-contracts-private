import { Address } from '../terra_utils';
import { env } from '../env_helper';
import { MsgExecuteContract } from "@terra-money/terra.js"

function getContractLog(response: any) {
  console.log(response);
  return response.logs[0].eventsByType.from_contract;
}

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);

  let p2p = handler.getContract(env.contracts.p2p)


  let createMsgs: MsgExecuteContract[] = []
  for(var i=0;i<100;i++){
     let msg = {
        create_trade:{
        }
      };
      let executeMsg = new MsgExecuteContract(
          handler.getAddress(), // sender
          p2p.address, // contract address
          { ...msg }, // handle msg,
        );
      createMsgs.push(executeMsg)
  }
  
  let response = await handler.post(createMsgs)

  console.log(response);

}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
