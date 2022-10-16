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
  let trade_id = 3;
  let response = await p2p.execute.add_whitelisted_users({
    trade_id,
    whitelisted_users: ["terra1hzttzrf2yge4pepnlalvt5zuaphpzk3nnc8x7s"]
  })
  console.log(response);

}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
