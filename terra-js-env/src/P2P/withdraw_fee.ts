import { Address } from '../terra_utils';
import { env } from '../env_helper';

function getContractLog(response: any) {
  console.log(response);
  return response.logs[0].eventsByType.from_contract;
}

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);

  let fee_distributor = handler.getContract(env.contracts.fee_distributor)
  // We try to add trades
  let response = await fee_distributor.execute.withdraw_fees({
    addresses: ["terra1pw2svczz3s0kgspc8ejm6lxlhmdskutynzfryre0me69kdj0u90qxhnvxj"]
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
