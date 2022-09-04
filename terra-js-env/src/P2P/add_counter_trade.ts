import { Address } from '../terra_utils';
import { env } from '../env_helper';

function getContractLog(response: any) {
  console.log(response);
  return response.logs[0].eventsByType.from_contract;
}

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);

  let p2p = handler.getContract(env.contracts.p2p)
  // We try to add trades
  let response = await p2p.execute.suggest_counter_trade({
    trade_id: 1
  });

  console.log(response);

}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
