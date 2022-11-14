import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  // Uploading the contract code
  let loan = handler.getContract(env.contracts.loan);


  await loan.execute.set_fee_distributor({
    fee_depositor: env.contracts.fee_distributor
  })
}

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
