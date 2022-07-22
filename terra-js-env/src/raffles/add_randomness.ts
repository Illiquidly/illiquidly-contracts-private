import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';
import { Numeric } from '@terra-money/terra.js';
function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

/// Here we want to upload the p2p contract and add the fee contract
async function main() {

  let handler = new Address(env['mnemonics'][0]);
  let anyOne = new Address(env['mnemonics'][1]);
  let raffle_contract = anyOne.getContract(env.contracts.raffle);

  // Test verification
  let pubkey = "868f005eb8e6e4ca0a47c8a77ceaa5309a47978a7c71bc5cce96366b5d7a569937c529eeda66c7293784a9402801af31"
  let randomness = {"round":2098475,"randomness":"89580f6a639add6c90dcf3d222e35415f89d9ee2cd6ef6fc4f23134cdffa5d1e","signature":"84bcb438f4505ef5282804c3d98a76b536970f9755004ea103fc15183c1b314a1e95582cb4dbaf3330272c72fe5675550ac6718ef53dfe18fca65c37f223a7dfbc1920a9c32dbf01a227290293a67fa4682dd266f717f5078829d83912926649","previous_signature":"b49ee4089fc510300b38d75ebba84576097bed61c171574acf2557f636c7144b471b57e18e5b0c3f8774e194344931c011873149d0db51fc70d22448bfc264d230be7ed6fcd3eb3b61fdc877d657dfa0d8ecaea6c1fa35f90bc84e88c1af17d4"}

  // From https://api.drand.sh/info
  let response = await raffle_contract.execute.update_randomness({
    randomness: {
      round: randomness.round,
      randomness: Buffer.from(randomness.randomness, 'hex').toString('base64'),
      signature: Buffer.from(randomness.signature, 'hex').toString('base64'),
      previous_signature: Buffer.from(randomness.previous_signature, 'hex').toString('base64'),
    },
    raffle_id: 0,

  })
  console.log(response)


}

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
