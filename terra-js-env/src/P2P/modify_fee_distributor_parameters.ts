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
  let response = await fee_distributor.execute.modify_contract_info({
        treasury: "terra1yttw08pl3y3txd3jls4pmw5n9pesggcnta3u87ak2tddk97satasvdul7n",
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
