# PaymentWithdrawRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**jwt** | Option<**String**> | JWT for Ledger API authentication | [optional]
**seller_party_id** | Option<**String**> | Seller party ID (e.g., \"ext-user-phantom-1::1220...\") | [optional]
**seller_private_key** | Option<**String**> | Base58-encoded Ed25519 private key | [optional]
**payment_cid** | **String** | Contract ID of AdvancedPayment | 
**amount** | **String** | Amount to withdraw (in CC) | 
**reason** | Option<**String**> | Withdrawal reason | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


