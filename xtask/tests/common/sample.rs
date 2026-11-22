//! The worked sample model of the slice 0074 design brief, section 3
//! (LIB-TOOL-004, LIB-TOOL-023): a hand-written UML model in the shape of
//! ptc/22-03-13, with no OMG content.
//!
//! One package, `Sample`, with an abstract class `Item` (a Data property
//! `label` [0..1]), two subclasses of it, `Leaf` (an Enumeration property
//! `state` [1..1] and two References typed `Item`) and `Box` (a Composition
//! `item` typed `Item` [0..*], a Composition `tag` typed `Tag` [0..1] and a
//! Reference `next` typed `Leaf` [2..*]), a class `Tag` with no properties,
//! and an Enumeration `State` with the literals `open` and `closed`. Its
//! schema, `SAMPLE_SCHEMA` in `schema.rs`, has SHA-256
//! `5a097b8ac180c4f8cac7ca3f3ff4c665c6b5c558cbc8184d131f79a17a2cabec`.
//!
//! Included with `#[path]` by the test files that use it.

/// The sample model, as the design brief gives it.
pub(crate) const SAMPLE_MODEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmi:XMI xmlns:uml="http://www.omg.org/spec/UML/20131001" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <uml:Model xmi:type="uml:Model" xmi:id="m" name="Model">
    <packagedElement xmi:type="uml:Package" xmi:id="p" name="Sample">
      <packagedElement xmi:type="uml:Class" xmi:id="Item" name="Item" isAbstract="true">
        <ownedAttribute xmi:type="uml:Property" xmi:id="Item.label" name="label">
          <type href="http://www.omg.org/spec/UML/20131001/PrimitiveTypes.xmi#String"/>
          <lowerValue xmi:type="uml:LiteralInteger" xmi:id="Item.label.lower"/>
        </ownedAttribute>
      </packagedElement>
      <packagedElement xmi:type="uml:Class" xmi:id="Leaf" name="Leaf">
        <generalization xmi:type="uml:Generalization" xmi:id="Leaf.general" general="Item"/>
        <ownedAttribute xmi:type="uml:Property" xmi:id="Leaf.state" name="state" type="State"/>
        <ownedAttribute xmi:type="uml:Property" xmi:id="Leaf.next" name="next" type="Item">
          <lowerValue xmi:type="uml:LiteralInteger" xmi:id="Leaf.next.lower"/>
        </ownedAttribute>
        <ownedAttribute xmi:type="uml:Property" xmi:id="Leaf.item" name="item" type="Item">
          <lowerValue xmi:type="uml:LiteralInteger" xmi:id="Leaf.item.lower"/>
        </ownedAttribute>
      </packagedElement>
      <packagedElement xmi:type="uml:Class" xmi:id="Box" name="Box">
        <generalization xmi:type="uml:Generalization" xmi:id="Box.general" general="Item"/>
        <ownedAttribute xmi:type="uml:Property" xmi:id="Box.item" name="item" type="Item" aggregation="composite">
          <lowerValue xmi:type="uml:LiteralInteger" xmi:id="Box.item.lower"/>
          <upperValue xmi:type="uml:LiteralUnlimitedNatural" xmi:id="Box.item.upper" value="*"/>
        </ownedAttribute>
        <ownedAttribute xmi:type="uml:Property" xmi:id="Box.tag" name="tag" type="Tag" aggregation="composite">
          <lowerValue xmi:type="uml:LiteralInteger" xmi:id="Box.tag.lower"/>
        </ownedAttribute>
        <ownedAttribute xmi:type="uml:Property" xmi:id="Box.next" name="next" type="Leaf">
          <lowerValue xmi:type="uml:LiteralInteger" xmi:id="Box.next.lower" value="2"/>
          <upperValue xmi:type="uml:LiteralUnlimitedNatural" xmi:id="Box.next.upper" value="*"/>
        </ownedAttribute>
      </packagedElement>
      <packagedElement xmi:type="uml:Class" xmi:id="Tag" name="Tag"/>
      <packagedElement xmi:type="uml:Enumeration" xmi:id="State" name="State">
        <ownedLiteral xmi:type="uml:EnumerationLiteral" xmi:id="State.open" name="open"/>
        <ownedLiteral xmi:type="uml:EnumerationLiteral" xmi:id="State.closed" name="closed"/>
      </packagedElement>
    </packagedElement>
  </uml:Model>
</xmi:XMI>
"#;
