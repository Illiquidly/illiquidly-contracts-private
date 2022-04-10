import { Address } from './terra_utils';
import { env } from './env_helper';

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);

  // Uploading the contract code
  let iliq_codeId: string[] = await handler.uploadContract(
    '../artifacts/iliq_token.wasm'
  );
  let multisender_codeId: string[] = await handler.uploadContract(
    '../artifacts/multisender.wasm'
  );

  // Instantiating the contract
  let iliqInitMsg = {
    custom: 'tesoutil',
    name: 'ILLIQUIDLY TOKEN',
    symbol: 'ILIQ',
    decimals: 6,
    initial_balances: [
      {
        address: handler.getAddress(),
        amount: '1000000'
      }
    ]
  };
  let iliq = await handler.instantiateContract(+iliq_codeId[0], iliqInitMsg);

  let multisenderInitMsg = {
    name: 'MULTISENDER'
  };
  let multisender = await handler.instantiateContract(
    +multisender_codeId[0],
    multisenderInitMsg
  );

  // Approving the multisender
  let response = await iliq.execute.increase_allowance({
    spender: multisender.address,
    amount: '1000'
  });
  console.log(response);

  // Testing the multisender
  let multi_response = await multisender.execute.send({
    to_send: [
      {
        cw20_coin: {
          address: iliq.address,
          amount: '500'
        }
      },
      {
        cw20_coin: {
          address: iliq.address,
          amount: '500'
        }
      }
    ],
    receivers: [
      'terra1vchq78v89nydypd3xn8hc6s2a28ks80fhtulfr',
      'terra1vchq78v89nydypd3xn8hc6s2a28ks80fhtulfr'
    ]
  });
  console.log(multi_response);
  console.log('AIURYIAUFYIDUVKQJDBsdghsgdfjhsgfsdf\nskdfjghsdkfguztriuazr');

  // Asserting side effects
  response = await iliq.query.balance({ address: handler.getAddress() });
  console.log(response);
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
