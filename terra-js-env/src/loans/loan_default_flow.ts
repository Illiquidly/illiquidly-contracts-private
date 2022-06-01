import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';
import { Numeric } from '@terra-money/terra.js';
function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

/// Here we want to upload the p2p contract and add the fee contract
async function main() {


  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let borrower = new Address(env['mnemonics'][1]);
  let lender = new Address(env['mnemonics'][2]);
  // Uploading the contract code
  let loan = borrower.getContract(env.contracts.loan);
  let loan_lender = lender.getContract(env.contracts.loan);
  let fee_distributor = handler.getContract(env.contracts.fee_distributor);
  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft = handler.getContract(cw721_tokens[cw721_token_names[0]]);
  let borrower_nft = borrower.getContract(cw721_tokens[cw721_token_names[0]]);
  let response = await nft.query.tokens({ owner: borrower.getAddress() });
  let token_id;
  console.log("Tokens available at the beggining", response)
  if (response.tokens.length == 0) {
    console.log('Mint new token');
    token_id = borrower.getAddress() + Math.ceil(Math.random() * 10000);
    await nft.execute.mint({
      token_id: token_id,
      owner: borrower.getAddress(),
      token_uri: 'testing'
    });
  } else {
    token_id = response.tokens[0];
  }

  // We save the initial balances
  let balance_borrower_before: Numeric.Output = (
    await borrower.terra.bank.balance(borrower.getAddress())
  )[0].get('uluna')!.amount;
  let balance_lender_before: Numeric.Output = (
    await lender.terra.bank.balance(lender.getAddress())
  )[0].get('uluna')!.amount;

  // We start the flow !!
  response = await borrower_nft.execute.approve({
    spender: loan.address,
    token_id: token_id
  });
  console.log('Approved nft');

  response = await loan.execute.deposit_collateral({
    address: nft.address,
    token_id: token_id
  });
  console.log('Deposited Collateral', token_id);
  let loan_id = parseInt(getContractLog(response).loan_id[0]);
  // As we deposit the collateral, the token shouldn't be available anymore to the borrower
  let token_ids_left = await nft.query.tokens({ owner: borrower.getAddress() });
  console.log("tokens left", token_ids_left);
  
  response = await loan_lender.execute.make_offer(
    {
      borrower: borrower.getAddress(),
      loan_id: loan_id,
      terms: {
        principle: {
          amount: '500',
          denom: 'uluna'
        },
        interest: '50',
        duration_in_blocks: 0
      }
    },
    '500uluna'
  );
  let offer_id = parseInt(getContractLog(response).offer_id[0]);
  let balance_lender_after: Numeric.Output = (
    await lender.terra.bank.balance(lender.getAddress())
  )[0].get('uluna')!.amount;

  console.log('Offer made, gave funds to the contract --> ', balance_lender_before.sub(balance_lender_after))

  response = await loan.execute.accept_offer({
    loan_id: loan_id,
    offer_id: offer_id
  });
  console.log('Offer accepted');

  let balance_borrower_after: Numeric.Output = (
    await borrower.terra.bank.balance(borrower.getAddress())
  )[0].get('uluna')!.amount;
  balance_lender_after = (
    await lender.terra.bank.balance(lender.getAddress())
  )[0].get('uluna')!.amount;
  console.log('Borrower balance difference : ', balance_borrower_after.sub(balance_borrower_before));
  console.log('Lender balance difference', balance_lender_after.sub(balance_lender_before));


  console.log("Sleeping for some time to make sure the loan is defaulted")

  await new Promise(r => setTimeout(r, 500));

  console.log("Trying to redeem the loan, but the time elapsed is too big, this should fail")
  // Finally my precious NFT
  await loan.execute.repay_borrowed_funds(
    {
      loan_id: loan_id
    },
    '550uluna'
  );


  await loan_lender.execute.withdraw_defaulted_loan({
    borrower: borrower.getAddress(),
    loan_id
  })
  console.log(
    'Withdrawn the funds from the defaulted loan'
  );

  balance_borrower_after = (
    await borrower.terra.bank.balance(borrower.getAddress())
  )[0].get('uluna')!.amount;
  balance_lender_after = (
    await lender.terra.bank.balance(lender.getAddress())
  )[0].get('uluna')!.amount;
  console.log('Borrower balance difference', balance_borrower_after.sub(balance_borrower_before));
  console.log('Lender balance difference', balance_lender_after.sub(balance_lender_before));


  // We verify the nft is now available to the lender
  response = await nft.query.tokens({ owner: lender.getAddress() });
  console.log(response);
}

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
