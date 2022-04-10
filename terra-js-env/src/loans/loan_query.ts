import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  // Uploading the contract code
  let loan = handler.getContract(env.contracts.loan);

  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft = handler.getContract(cw721_tokens[cw721_token_names[0]]);
  // We verify the nft is still available
  let response = await nft.query.tokens({ owner: handler.getAddress() });
  console.log(response);

  response = await loan.query.collateral_info({
    borrower: handler.getAddress(),
    loan_id: 0
  });
  console.log(response);

  response = await loan.query.contract_info();
  console.log(response);
}

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
