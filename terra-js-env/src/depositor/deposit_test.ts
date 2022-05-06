import { Address } from '../terra_utils';
import { env } from '../env_helper';

function getContractLog(response: any) {
  console.log(response);
  return response.logs[0].eventsByType.from_contract;
}


async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);

  let fee_distributor = handler.getContract(env.contracts.fee_distributor);
  console.log(await fee_distributor.query.contract_info());
  let response = await fee_distributor.execute.deposit_fees({
    addresses: ["tr"],
  }, "150uluna")
  console.log(response);
  console.log(await fee_distributor.query.amount({
    address: handler.getAddress()
  }));

  console.log(await fee_distributor.query.addresses());
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
