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
  let response = await fee_distributor.execute.add_associated_address({
        address: "terra1vn0qwkp9l53q73ajsrnexdw97ekzscexh2q5rduk2kajqrvzwtkqj4nc08",
        fee_address: "terra1rhfcc28fu2dev0r9d20z3g38ewpg2cpr9lglrc",
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
