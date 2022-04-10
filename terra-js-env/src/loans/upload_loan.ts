import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  // Uploading the contract code
  let loan_codeId: string[] = await handler.uploadContract(
    '../artifacts/nft_loans.wasm'
  );

  // Initialize p2p contract
  let loanInitMsg = {
    name: 'P2PLoans',
    treasury: handler.getAddress(),
    fee_rate: '5000'
  };

  let loan = await handler.instantiateContract(+loan_codeId[0], loanInitMsg);
  add_contract('loan', loan.address);

  console.log('Uploaded the loan contract');
}

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
