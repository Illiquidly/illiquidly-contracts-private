import { Address } from '../terra_utils';
import { env, add_contract } from '../env_helper';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  // Uploading the contract code
  let codeId: string[] = await handler.uploadContract(
    '../artifacts/raffles.wasm'
  );

  let chain_hash = "8990e7a9aaed2ffed73dbd7092123d6f289930540d7651336225dc172e51b2ce";
  let hexPubkey = "868f005eb8e6e4ca0a47c8a77ceaa5309a47978a7c71bc5cce96366b5d7a569937c529eeda66c7293784a9402801af31";
  // Initialize Raffle contract
  let initMsg = {
    name: 'NFTRaffles',
    random_pubkey: Buffer.from(hexPubkey, 'hex').toString('base64'),
    chain_hash,
    verify_signature_contract : env.contracts.raffle_verifier,
    max_participant_number: 5000
  };
  console.log(initMsg)

  let contract = await handler.instantiateContract(+codeId[0], initMsg);
  add_contract('raffle', contract.address);

  console.log('Uploaded the raffle contract');
}

main()
  .then(() => {})
  .catch((err) => {
    console.log(err);
  });
