import { Address } from './terra_utils';
import { env } from './env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';

function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  //let counter = new Address(env['mnemonics'][1]);
  console.log(handler.getAddress());
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
